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
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListMinMax.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoReverse.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFirst.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLast.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSort.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAppend.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoin.java
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
//!
//! # Where a new command goes
//!
//! A command that expressions compute themselves (`If`, `Sequence`, `Sum`,
//! `Mod`, ...) lives in `commands/<group>.rs`, where `<group>` is its
//! `group` column in `docs/command-matrix.csv`, and its tests in
//! `commands/<group>/tests.rs`. The group file holds its name, how it is
//! parsed, its type and its evaluation; `parse`, `types` and `eval` only
//! ask the groups in turn (see `commands/mod.rs`). Its node is a variant of
//! [`ListOp`] (a command of a list) or [`Bin`] (a command of two numbers),
//! added in the enum's block of its group; a command of another shape needs
//! a new [`Node`] first. A group without a file yet adds one and registers
//! it in `commands/mod.rs`.

mod cas;
mod commands;

/// The order of `GeoText.getComparator`: texts compared without accents,
/// then as they are, both by UTF-16 units (as `Sort` of texts orders them).
pub(crate) fn text_order(a: &str, b: &str) -> std::cmp::Ordering {
    use commands::text::strings::{java_cmp, remove_accents};
    java_cmp(&remove_accents(a), &remove_accents(b)).then_with(|| java_cmp(a, b))
}
pub(crate) use commands::brent_minimum;
mod decimal;
pub(crate) mod derivative;
/// Equations in `x` and `y` as lines and conics (see the module docs).
pub(crate) mod equation;
mod eval;
pub(crate) mod fraction;
pub(crate) mod inequality;
mod inverse;
pub(crate) use eval::{carrier_deadline, charge_carrier};
pub(crate) use eval::{reserve_work, with_work_deadline, work_deadline};
#[doc(hidden)]
pub use eval::{work_done, MAX_ELEMENTS, MAX_EVALUATION_WORK, MAX_OVERRUN};

/// Shared expression-work allowance for one intersection solve.
pub(crate) struct IntersectionWork(eval::Lists);

impl IntersectionWork {
    pub(crate) fn new() -> Self {
        Self(eval::Lists::new(Vec::new()))
    }

    pub(crate) fn charge(&self, units: u64) -> bool {
        self.0.charge(units)
    }

    pub(crate) fn sample_system(&self, c: &Construction, id: Id, t: f64, state: &[f64]) -> f64 {
        if !self.charge(1) {
            return f64::NAN;
        }
        system_value(c, id, t, state, &self.0).num()
    }

    pub(crate) fn exhausted(&self) -> bool {
        self.0.over()
    }

    /// The subtree `i` of the two-variable function `e` at `(x, y)`, with
    /// the function's first two locals bound to the coordinates.
    pub(crate) fn sample_plane(&self, c: &Construction, e: &Entry, i: u32, x: f64, y: f64) -> f64 {
        if !self.charge(1) {
            return f64::NAN;
        }
        let env = eval::Env::Pair(x, y);
        let ctx = eval::Ctx::<true> {
            meter: Some(&self.0),
            ..eval::Ctx::new(c, e, &env)
        };
        let first = eval::Local {
            depth: 0,
            value: eval::Val::N(x),
            run: None,
            vector: false,
            text: None,
            up: None,
        };
        let second = eval::Local {
            depth: 1,
            value: eval::Val::N(y),
            run: None,
            vector: false,
            text: None,
            up: Some(&first),
        };
        eval::Ctx {
            locals: Some(&second),
            ..ctx
        }
        .fast(i)
    }
}

fn system_value(c: &Construction, id: Id, t: f64, state: &[f64], meter: &eval::Lists) -> eval::Val {
    use eval::Val::N;
    let Def::Expr(d) = c.def(id) else {
        return N(f64::NAN);
    };
    let function = entry(c, d);
    let env = eval::Env::Pair(t, state.first().copied().unwrap_or(0.0));
    let ctx = eval::Ctx::<true> {
        meter: Some(meter),
        ..eval::Ctx::new(c, function, &env)
    };
    match (c.value(id), ctx.node(function.root)) {
        (Value::FunctionNVar, Node::SystemFunction(arity, f)) => {
            let mut args = [0.0; 16];
            let n = usize::from(arity.saturating_sub(1));
            if n == 0 || n > args.len() {
                return N(f64::NAN);
            }
            let supplied = state.len().min(n);
            args[..supplied].copy_from_slice(&state[..supplied]);
            let time = eval::Local {
                depth: 0,
                value: N(t),
                run: None,
                vector: false,
                text: None,
                up: None,
            };
            eval::system_locals(
                &eval::Ctx {
                    locals: Some(&time),
                    ..ctx
                },
                f,
                1,
                &args[..n],
            )
        }
        (Value::Function, _) => {
            if function.function_of_y {
                let env = eval::Env::Value(state.first().copied().unwrap_or(f64::NAN));
                N(eval::Ctx { env: &env, ..ctx }.fast(function.root))
            } else {
                N(ctx.fast(function.root))
            }
        }
        (Value::FunctionNVar, Node::Cas(..)) => N(ctx.fast(function.root)),
        (Value::FunctionNVar, _) => {
            N(
                equation::sample_side(c, id, t, state.first().copied().unwrap_or(0.0), meter)
                    .unwrap_or(f64::NAN),
            )
        }
        _ => N(f64::NAN),
    }
}

pub(crate) mod lex;
pub(crate) use lex::{is_name_char, plain_number};
mod list;
pub(in crate::expr) use commands::text::TextOp;
pub(crate) use list::{same_slots, Slot};
pub use list::{Element, List};
pub(crate) mod math;
mod names;
mod parse;
pub(crate) use parse::{Parsed, MAX_DEPTH};
mod types;
mod wide;

use crate::algo::intersect::Hint;
use crate::algo::vector::VectorDef;
use crate::kernel::angle::AngleStyle;
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use decimal::Dec;

/// Handle of an expression object's tree, valid only in the construction
/// that made it; see the module documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprDef {
    kind: Kind,
    pub(crate) angle_style: AngleStyle,
    allows_infinite_point: bool,
    index: u32,
}

impl ExprDef {
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

/// One node of an expression tree. Children are indices into the same
/// arena and always smaller than the index of their parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Node {
    /// A constant (`MyDouble`): a literal, `π`, `e`, `∞` or `?`.
    Num(f64),
    /// The factor GeoGebra puts in front of a negated term.
    MinusOne,
    /// The degree sign, `π/180` with one angle dimension.
    Degree,
    Obj(Id),
    /// The variable `x` of a function, and of an equation in `x` and `y`
    /// (see [`equation`]).
    Var,
    /// The variable `y` of an equation in `x` and `y`; GeoGebra's
    /// `FunctionVariable` "y", which the parser of an equation reads
    /// (`Equation.initEquation`). Never in the tree of a function or a
    /// plain expression: only [`equation`] makes this node, and it leaves
    /// no `y` in the coefficient trees it builds.
    VarY,
    /// A command computed inside the expression, with the intersection
    /// hint it got when the expression was parsed.
    Cmd(Def, Hint),
    /// `(x, y)`.
    Tuple(u32, u32),
    /// `(r; φ)`.
    Polar(u32, u32),
    /// Inline vector command with a coordinate expression.
    Vector(u32),
    Un(Un, u32),
    Bin(Bin, u32, u32),
    Algebra(AlgebraOp, u32, u32),
    Cas(cas::Op, u32, u32),
    CasVariable(u8),
    Text,
    TextCmd(TextOp, u32, u32),
    /// A financial command (`Rate`, `Periods`, ...) of three to six
    /// numbers; the argument roots fill the array from the front, the rest
    /// stays [`NONE`].
    Financial(Finance, [u32; 6]),
    Relation {
        objects: [Id; 4],
        count: u8,
        list: Option<Id>,
    },
    Base {
        to: bool,
        value: u32,
        radix: u32,
    },
    /// A numerical extremum on a bounded interval. The function's x is local.
    Extremum(bool, u32, u32, u32),
    RootList(u32),
    FunctionAsymptotes(u32),
    VertexForm(u32),
    Iteration {
        mode: IterationMode,
        collect: bool,
        formula: u32,
        initial: u32,
        count: u32,
    },
    /// Quotient or remainder of two polynomial functions of the same x.
    PolynomialPart(bool, u32, u32),
    /// Reference to a function as a list element.
    FunctionObject(Id),
    SystemFunction(u8, u32),
    /// A function object applied to an argument.
    Apply(Id, u32),
    Apply2(Id, u32, u32),
    Side(Id, bool),
    /// The symbolic inverse of a function expression in `x`.
    Inverse(u32),
    /// Dilation of a function or list by a factor about an optional point.
    Dilate(u32, u32, u32),
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
    /// A named geometric object in a list literal.
    ItemObject(Id),
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
    /// A list command of more arguments than [`Node::ListOp`] holds: its
    /// `len` arguments are the nodes from `first` on (`Take`, `Zip`, ...).
    Call(CallOp, u32, u32),
    /// The variable of the enclosing `Sequence` of this depth (0 for the
    /// outermost).
    Local(u32, Elem),
    Filter {
        keep: bool,
        list: u32,
        condition: u32,
        local: u32,
        elem: Elem,
        explicit: bool,
    },
    Defined(u32, bool),
    ObjectDefined(Id),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum IterationMode {
    Unary,
    Indexed,
    Local { first: u32, count: u32 },
}

/// Numeric commands with a scalar or list result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlgebraOp {
    Division,
    PrimeFactors,
    Divisors,
    DivisorsList,
    DivisorsSum,
    IsPrime,
    IsInteger,
}

impl AlgebraOp {
    fn makes_list(self) -> bool {
        matches!(
            self,
            Self::Division | Self::PrimeFactors | Self::DivisorsList
        )
    }
}

/// The commands behind a [`Node::Call`] (see there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CallOp {
    /// `Take(list, from[, to])` (`AlgoTake`).
    Take,
    /// `IndexOf(object, list[, start])` (`AlgoIndexOf`).
    IndexOf,
    /// `Insert(object, list, position)` (`AlgoInsert`).
    Insert,
    /// `Classes(list, number of classes | start, width)` (`AlgoClasses`).
    Classes,
    /// `Frequency(...)` (`AlgoFrequency`).
    Frequency,
    /// `Zip(expression, variable, list, ...)` (`AlgoZip`): the expression,
    /// then a [`Node::Local`] and its list per variable.
    Zip,
    /// `Product(expression, variable, from, to)` (`AlgoFoldExpression`).
    Fold,
    /// `Sum(expression, variable, from, to)` (`AlgoFoldExpression` with
    /// `PLUS`, `CmdSum.processSymb`): the same shape, added up.
    SumFold,
}

/// The commands that read a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListOp {
    // the list group (`commands/list.rs`)
    /// `l(n)`: rounded, from 1; a negative index counts from the end
    /// (`ExpressionNodeEvaluator.handleElementOf`).
    Index,
    /// `Element(l, n)`: rounded, from 1 (`AlgoListElement`).
    Element,
    /// `Product(list)` and `Product(list, n)` (`AlgoStats1D`).
    Product,
    /// `Reverse(list)` (`AlgoReverse`).
    Reverse,
    /// `First(list)`, `First(list, n)` and `Last` (`AlgoFirst`, `AlgoLast`).
    First,
    Last,
    /// `First(Function(list), n)`: the points of the freehand function of
    /// the list written inline (`AlgoFirstLocus`).
    FirstFreehand,
    /// `Sort(list)` of numbers or points (`AlgoSort`).
    Sort,
    /// `Append(list, object)` and `Append(object, list)`: the second
    /// operand is the object (`AlgoAppend`).
    Append,
    Prepend,
    /// `Join(list of lists)`; `Join(l1, l2, ...)` joins the list of its
    /// arguments (`AlgoJoin`).
    Join,
    /// `Take` is a [`Node::Call`]; `RemoveUndefined(list)`
    /// (`AlgoRemoveUndefined`).
    RemoveUndefined,
    /// `Unique(list)` (`AlgoUnique`).
    Unique,
    /// `Flatten(list)` (`AlgoFlatten`).
    Flatten,
    /// `PointList(list)` (`AlgoPointList`).
    PointList,
    /// `OrdinalRank(list)` (`AlgoOrdinalRank`).
    OrdinalRank,
    /// `TiedRank(list)` (`AlgoTiedRank`).
    TiedRank,
    /// `Union(l1, l2)` (`AlgoUnion`); `Intersection(l1, l2)`
    /// (`AlgoIntersection`); `Remove(l1, l2)` (`AlgoRemove`).
    Union,
    Intersect,
    Remove,

    // statistics (`commands/statistics.rs`)
    /// `Sum(list)` and `Sum(list, n)` of numbers (`AlgoStats1D`).
    Sum,
    /// `Mean(list)` (`AlgoStats1D`).
    Mean,

    // algebra (`commands/algebra.rs`)
    /// `Min(list)` and `Max(list)` (`AlgoListMinMax`).
    Min,
    Max,
    Gcd,
    Lcm,

    // geometry (`commands/geometry.rs`)
    /// `Length(list)`.
    Length,
    /// `Dot(list, list)` and `Cross(list, list)`.
    Dot,
    Cross,
    /// `Determinant(matrix)` and `MatrixRank(matrix)` inside expressions.
    Determinant,
    Normalize,
    Svd,
    MatrixRank,
    Transpose,
    Rref,
    Invert,
    Dimension,
    MatrixDimension,
    Identity,
    ApplyMatrix,
    ApplyMatrixCoordinates {
        vector: bool,
    },
}

impl ListOp {
    /// Whether the result is a list.
    pub(crate) fn makes_list(self) -> bool {
        commands::makes_list(self)
    }
}

/// No node: the missing else branch of an `If`.
pub(crate) const NONE: u32 = u32::MAX;

/// How an `If` decides (`CmdIf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IfMode {
    /// `AlgoIf`, the command of an expression that is not a function of
    /// `x` in its conditions: the first true condition picks its branch;
    /// an undefined condition is false; no true condition and no else
    /// branch is undefined. The result type widens in argument order.
    Algo,
    /// Another condition in the same `AlgoIf` call.
    AlgoNext,
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
    Length,
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

/// The five financial commands of `AlgoFinancial`: which of the five
/// values the fundamental equation is solved for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Finance {
    Rate,
    Periods,
    Payment,
    PresentValue,
    FutureValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bin {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    RootPow,
    Atan2,
    /// `log(b, x)`: the base comes first.
    LogB,
    Round2,
    /// Number of terms in a fold whose expression is independent of the index.
    FoldCount,
    /// The commands of two numbers (see [`commands`]).
    Mod,
    /// `Div`, the integer quotient.
    IntDiv,
    Gcd,
    Lcm,
    CommonDenominator,
    Max,
    Min,
    /// `nCr(n, r)`, the binomial coefficient (`MyMath.binomial`).
    Ncr,
    /// `nPr(n, r)`, the number of permutations (`AlgoNpR.nPr`).
    Npr,
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
        commands::is_binary_command(self)
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
    Function,
    Text,
    List(Elem),
    Object,
}

/// The type of the elements of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Elem {
    Num,
    /// Points and vectors.
    Vec,
    Bool,
    Function,
    Text,
    List,
    /// Named geometric objects.
    Object,
    /// No element: `{}`.
    Empty,
    /// Elements of different types, such as `{1, (1, 2)}`.
    Any,
}

/// The last result of an expression, with infinities kept: GeoGebra keeps
/// them in a dependent number (`1/(1/0)` is 0) and in the coordinates of a
/// vector, which shows as undefined. A boolean is kept as 1 or 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Raw {
    Num(f64),
    Vec(f64, f64),
    /// Homogeneous coordinates from a projective point transform.
    Homogeneous([f64; 3]),
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
            (Raw::Homogeneous(a), Raw::Homogeneous(b)) => {
                a.map(f64::to_bits) == b.map(f64::to_bits)
            }
            (Raw::List(a), Raw::List(b)) => a == b,
            _ => false,
        }
    }
}

/// What GeoGebra's number object of a constant node is: a `MySpecialDouble`
/// (a literal or a named constant, with the decimal it was written as, see
/// [`decimal`]) or, by default, a plain `MyDouble` that the parser made.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Lit {
    #[default]
    Plain,
    // XML definitions preserve the spelling of input literals, including
    // trailing zeros that the exact decimal representation normalizes away.
    Special(Option<Dec>, Option<std::sync::Arc<str>>),
    Text(std::sync::Arc<str>),
}

impl Lit {
    fn special(&self) -> bool {
        matches!(self, Self::Special(..))
    }

    fn decimal(&self) -> Option<&Dec> {
        match self {
            Self::Special(decimal, _) => decimal.as_ref(),
            _ => None,
        }
    }
}

/// A number, text or polynomial result can store one additional value.
/// Keeping the variants together avoids enlarging ordinary expression entries.
#[derive(Clone, Default, PartialEq)]
enum StoredExtra {
    #[default]
    None,
    Double,
    Decimal(Dec),
    Text(Box<TextResult>),
    Polynomial(Vec<f64>),
    Cas(std::sync::Arc<cas::FunctionResult>),
}

impl StoredExtra {
    fn exact(&self) -> Option<eval::Exact> {
        match self {
            Self::Double => Some(eval::Exact::Double),
            Self::Decimal(d) => Some(eval::Exact::Dec(d.clone())),
            _ => None,
        }
    }
}

impl From<Option<eval::Exact>> for StoredExtra {
    fn from(exact: Option<eval::Exact>) -> Self {
        match exact {
            None => Self::None,
            Some(eval::Exact::Double) => Self::Double,
            Some(eval::Exact::Dec(d)) => Self::Decimal(d),
        }
    }
}

#[derive(Clone, PartialEq)]
struct TextResult {
    text: Option<String>,
    presentation: crate::kernel::text::TextPresentation,
    invalid_transform_list: bool,
}

#[derive(Clone, Copy)]
struct FunctionInterval {
    bounds: [u32; 2],
    range: Option<[f64; 2]>,
    defined: bool,
}

type NestedTransforms = std::collections::BTreeMap<u32, (u32, Vec<Slot>)>;

#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) nodes: Vec<Node>,
    lits: Vec<Lit>,
    tys: Vec<Ty>,
    pub(crate) root: u32,
    parents: Vec<Id>,
    /// Whether this is a function.
    function: bool,
    /// Whether a one-variable function uses the vertical coordinate.
    function_of_y: bool,
    /// The name used by a Cartesian curve's coordinate function.
    curve_parameter: Option<Box<str>>,
    /// Explicit domain of a function loaded from a legacy Function command.
    interval: Option<FunctionInterval>,
    /// Whether a function's tree requires checking its branches or transforms
    /// to decide whether it is defined (see [`inputs_defined`]).
    checks_branches: bool,
    /// Whether this tree or an applied function needs an expression work meter.
    inverts: bool,
    /// The objects named in the tree (a function's inputs).
    inputs: Vec<Id>,
    /// The last result; for a function, 0 when it is defined and `NaN`
    /// when not.
    raw: Raw,
    extra: StoredExtra,
    /// Whether the tree has a list in it, which its evaluation builds in a
    /// scratch buffer.
    lists: bool,
    /// The elements of a list result (see [`list`]).
    items: Vec<Slot>,
    nested_transforms: NestedTransforms,
    coordinate_layout: Option<list::CoordinateLayout>,
    depth: u32,
    cost: u64,
    pub(crate) vars: u64,
    /// The operands the symbolic prover replaced with segments, by node
    /// index, with the endpoints of each (see [`replace_distance_operands`]).
    segment_operands: Vec<(u32, [Id; 2])>,
    /// The command nodes of this tree that went undefined for good.
    stuck: Stuck,
    list_history: ListHistory,
}

/// Nodes of algorithms that fill their result after an invalid input
/// without marking it defined again (`AlgoSVD` and `AlgoIdentity`).
/// A replacement definition starts with a fresh tree.
#[derive(Default)]
pub(crate) struct Stuck(std::sync::Mutex<Vec<u32>>);

impl Stuck {
    fn nodes(&self) -> std::sync::MutexGuard<'_, Vec<u32>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn contains(&self, node: u32) -> bool {
        self.nodes().contains(&node)
    }

    pub(super) fn insert(&self, node: u32) {
        let mut nodes = self.nodes();
        if !nodes.contains(&node) {
            nodes.push(node);
        }
    }
}

/// The nodes of `d`'s tree that went undefined for good (see [`Stuck`]).
pub(crate) fn stuck_nodes(c: &Construction, d: ExprDef) -> Vec<u32> {
    entry(c, d).stuck.nodes().clone()
}

/// Puts back what [`stuck_nodes`] returned for `d`.
pub(crate) fn restore_stuck_nodes(c: &Construction, d: ExprDef, nodes: Vec<u32>) {
    *entry(c, d).stuck.nodes() = nodes;
}

impl Clone for Stuck {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(self.nodes().clone()))
    }
}

/// A list helper owns its copied elements even when its defined flag is
/// cleared. Arithmetic needs the element kinds but never their old values.
/// Transpose also keeps its prior flag when clearing an empty result.
#[derive(Clone, Debug, Default)]
pub(crate) struct ListRecords {
    shapes: std::collections::HashMap<u32, std::sync::Arc<[Kind]>>,
    elements: usize,
    invalid_transposes: std::collections::HashSet<u32>,
    revision: u64,
}

#[derive(Default)]
struct ListHistory(std::sync::Mutex<ListRecords>);

impl ListHistory {
    fn records(&self) -> std::sync::MutexGuard<'_, ListRecords> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Clone for ListHistory {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(self.records().clone()))
    }
}

pub(crate) fn list_records(c: &Construction, d: ExprDef) -> ListRecords {
    entry(c, d).list_history.records().clone()
}

pub(crate) fn list_record_revision(c: &Construction, d: ExprDef) -> Option<u64> {
    let e = entry(c, d);
    e.lists.then(|| e.list_history.records().revision)
}

pub(crate) fn restore_list_records(c: &Construction, d: ExprDef, records: ListRecords) {
    *entry(c, d).list_history.records() = records;
}

pub(crate) fn reset_list_records(c: &Construction, d: ExprDef) -> bool {
    let e = entry(c, d);
    if !e.lists {
        return false;
    }
    let mut records = e.list_history.records();
    if records.shapes.is_empty() && records.invalid_transposes.is_empty() {
        return false;
    }
    *records = ListRecords {
        revision: records.revision.wrapping_add(1),
        ..ListRecords::default()
    };
    true
}

pub(crate) fn reset_transpose_flags(c: &Construction, d: ExprDef) -> bool {
    let e = entry(c, d);
    if !e.lists {
        return false;
    }
    let mut records = e.list_history.records();
    if records.invalid_transposes.is_empty() {
        return false;
    }
    records.invalid_transposes.clear();
    records.revision = records.revision.wrapping_add(1);
    true
}

impl Entry {
    fn kept_list_shape(&self, node: u32) -> Option<std::sync::Arc<[Kind]>> {
        self.list_history
            .records()
            .shapes
            .get(&node)
            .cloned()
            .or_else(|| {
                // AlgoTake starts with an empty GeoList before its first update.
                matches!(self.nodes[node as usize], Node::Call(CallOp::Take, ..))
                    .then(std::sync::Arc::default)
            })
    }

    fn evaluate_interval(&self, c: &Construction) -> Option<FunctionInterval> {
        let mut interval = self.interval?;
        let [min, max] = interval
            .bounds
            .map(|bound| sample_subtree(c, self, bound, 0.0));
        interval.defined = min <= max;
        // AlgoFunctionInterval keeps the previous domain when bounds reverse.
        if min.partial_cmp(&max) != Some(std::cmp::Ordering::Greater) {
            interval.range = interval.defined.then_some([min, max]);
        }
        Some(interval)
    }

    fn new(c: &Construction, p: parse::Parsed, function: bool) -> Self {
        let checks_branches = function
            && p.nodes.iter().any(|n| {
                matches!(
                    n,
                    Node::If(..)
                        | Node::Inverse(_)
                        | Node::Dilate(..)
                        | Node::PolynomialPart(..)
                        | Node::Cas(..)
                        | Node::Un(Un::Length, _)
                )
            });
        let inverts = p.nodes.iter().any(|node| match *node {
            Node::Inverse(_) | Node::Cas(..) => true,
            Node::Apply(f, _) | Node::Apply2(f, ..) => {
                matches!(c.def(f), Def::Expr(d) if entry(c, d).inverts)
            }
            _ => false,
        });
        let lists = reads_list(&p.nodes, &p.tys);
        Entry {
            stuck: Stuck::default(),
            list_history: ListHistory::default(),
            checks_branches,
            inverts,
            lists,
            items: Vec::new(),
            nested_transforms: NestedTransforms::new(),
            coordinate_layout: None,
            interval: None,
            nodes: p.nodes,
            lits: p.lits,
            tys: p.tys,
            root: p.root,
            parents: p.parents,
            function,
            function_of_y: false,
            curve_parameter: None,
            inputs: p.inputs,
            raw: Raw::Num(f64::NAN),
            extra: StoredExtra::None,
            depth: p.depth,
            cost: p.cost,
            vars: p.vars,
            segment_operands: Vec::new(),
        }
    }

    fn text(&self) -> Option<&str> {
        match &self.extra {
            StoredExtra::Text(result) => result.text.as_deref(),
            _ => None,
        }
    }

    fn text_latex(&self) -> bool {
        matches!(&self.extra, StoredExtra::Text(result) if result.presentation.latex)
    }

    fn polynomial(&self) -> Option<&[f64]> {
        match &self.extra {
            StoredExtra::Polynomial(coefficients) => Some(coefficients),
            _ => None,
        }
    }

    fn cas_root(&self) -> Option<(u32, cas::Op, u32, u32)> {
        let root = match self.nodes[self.root as usize] {
            Node::SystemFunction(_, inner) => inner,
            _ => self.root,
        };
        match self.nodes[root as usize] {
            Node::Cas(op, a, b) if op.is_function() => Some((root, op, a, b)),
            _ => None,
        }
    }

    pub(crate) fn cas_quotient(&self, i: u32) -> Option<&crate::cas::Quotient> {
        let StoredExtra::Cas(form) = &self.extra else {
            return None;
        };
        (self.cas_root()?.0 == i).then(|| form.quotient()).flatten()
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
        angle_style: AngleStyle,
        function_of_y: bool,
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
    let mut fixed = [NONE; 6];
    let mut run = 0..0;
    match *node {
        Node::Tuple(a, b)
        | Node::Polar(a, b)
        | Node::Bin(_, a, b)
        | Node::ListOp(_, a, b)
        | Node::Algebra(_, a, b)
        | Node::PolynomialPart(_, a, b)
        | Node::Cas(_, a, b)
        | Node::Apply2(_, a, b) => {
            fixed[..2].copy_from_slice(&[a, b]);
        }
        Node::Un(_, a) | Node::Apply(_, a) | Node::Item(a) | Node::Inverse(a) | Node::Vector(a) => {
            fixed[0] = a;
        }
        Node::If(_, a, b, c)
        | Node::Range(a, b, c)
        | Node::Dilate(a, b, c)
        | Node::Extremum(_, a, b, c) => fixed[..3].copy_from_slice(&[a, b, c]),
        Node::Base { value, radix, .. } => fixed[..2].copy_from_slice(&[value, radix]),
        Node::Filter {
            list, condition, ..
        } => fixed[..2].copy_from_slice(&[list, condition]),
        Node::Defined(a, _)
        | Node::RootList(a)
        | Node::FunctionAsymptotes(a)
        | Node::VertexForm(a)
        | Node::SystemFunction(_, a) => fixed[0] = a,
        Node::Iteration {
            formula,
            initial,
            count,
            ..
        } => {
            fixed[..3].copy_from_slice(&[formula, initial, count]);
        }
        Node::Seq(a, b, c, d, _) => fixed[..4].copy_from_slice(&[a, b, c, d]),
        Node::Financial(_, args) => fixed.copy_from_slice(&args),
        Node::List(first, len) | Node::TextCmd(_, first, len) | Node::Call(_, first, len) => {
            run = first..first.saturating_add(len)
        }
        _ => {}
    }
    fixed.into_iter().filter(|&c| c != NONE).chain(run)
}

/// Adds `by` to every child index of `node`, in place: the two sides of an
/// equation are parsed into one arena, so the nodes of the second keep
/// pointing at their children after they follow the first side's (see
/// [`equation`]). [`NONE`] stays.
pub(crate) fn shift_children(node: &mut Node, by: u32) {
    let shift = |c: &mut u32| {
        if *c != NONE {
            *c += by;
        }
    };
    match node {
        Node::Tuple(a, b)
        | Node::Polar(a, b)
        | Node::Bin(_, a, b)
        | Node::ListOp(_, a, b)
        | Node::Algebra(_, a, b)
        | Node::PolynomialPart(_, a, b)
        | Node::Cas(_, a, b)
        | Node::Apply2(_, a, b) => {
            shift(a);
            shift(b);
        }
        Node::Un(_, a) | Node::Apply(_, a) | Node::Item(a) | Node::Inverse(a) | Node::Vector(a) => {
            shift(a);
        }
        Node::If(_, a, b, c)
        | Node::Range(a, b, c)
        | Node::Dilate(a, b, c)
        | Node::Extremum(_, a, b, c) => {
            shift(a);
            shift(b);
            shift(c);
        }
        Node::Base { value, radix, .. } => {
            shift(value);
            shift(radix);
        }
        Node::Filter {
            list, condition, ..
        } => {
            shift(list);
            shift(condition);
        }
        Node::Defined(a, _)
        | Node::RootList(a)
        | Node::FunctionAsymptotes(a)
        | Node::VertexForm(a)
        | Node::SystemFunction(_, a) => shift(a),
        Node::Iteration {
            formula,
            initial,
            count,
            ..
        } => {
            shift(formula);
            shift(initial);
            shift(count);
        }
        Node::Seq(a, b, c, d, _) => {
            shift(a);
            shift(b);
            shift(c);
            shift(d);
        }
        Node::List(first, _) | Node::TextCmd(_, first, _) | Node::Call(_, first, _) => shift(first),
        Node::Financial(_, args) => args.iter_mut().for_each(shift),
        _ => {}
    }
}

/// Whether a node makes or reads a list, which a command does in GeoGebra.
fn is_list_node(node: &Node) -> bool {
    matches!(
        node,
        Node::List(..)
            | Node::TextCmd(..)
            | Node::Item(_)
            | Node::ItemObject(_)
            | Node::ListOp(..)
            | Node::Range(..)
            | Node::Seq(..)
            | Node::Call(..)
            | Node::Local(..)
            | Node::Filter { .. }
            | Node::Algebra(..)
            | Node::Extremum(..)
            | Node::RootList(..)
            | Node::FunctionAsymptotes(..)
            | Node::VertexForm(..)
            | Node::Iteration { .. }
            | Node::Base { .. }
            | Node::Apply2(..)
    )
}

/// Whether a tree makes or reads a list: a list node, or any subtree whose
/// type is a list, such as a list object read by `l - A`.
fn reads_list(nodes: &[Node], tys: &[Ty]) -> bool {
    nodes.iter().any(is_list_node) || tys.iter().any(|t| matches!(t, Ty::List(_)))
}

/// The commands that expressions compute themselves; `Length` and `Sum`
/// also of a list.
pub(crate) use commands::polynomial::division_list;

pub(crate) fn is_expression_command(name: &str) -> bool {
    commands::is_command(name)
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

pub(crate) fn replace_free_decimal(c: &mut Construction, id: Id, decimal: Option<Dec>) {
    if let Some(decimal) = decimal {
        set_free_decimal(c, id, decimal);
    } else {
        forget_decimal(c, id);
    }
}

/// Forgets the exact decimal of a free number that is being changed
/// (`GeoNumeric.setValue`); whether it had one.
pub(crate) fn forget_decimal(c: &mut Construction, id: Id) -> bool {
    c.exprs.free.get_mut(id.0).and_then(Option::take).is_some()
}

pub(crate) fn independent_definition(c: &Construction, def: Def) -> bool {
    match def {
        Def::Expr(d) => entry(c, d).inputs.is_empty(),
        _ => def.is_free(),
    }
}

/// Equality of parsed definitions in different expression slots. Ignore
/// source formatting, but retain the literal spelling used by XML definitions.
pub(crate) fn same_definition(c: &Construction, a: Def, b: Def) -> bool {
    match (a, b) {
        (Def::Expr(a), Def::Expr(b)) if a.kind == b.kind && a.angle_style == b.angle_style => {
            let a = entry(c, a);
            let b = entry(c, b);
            a.root == b.root
                && a.nodes.len() == b.nodes.len()
                && a.nodes.iter().zip(&b.nodes).all(|(a, b)| {
                    // Both undefined literals print as '?' in the definition.
                    a == b || matches!((a, b), (Node::Num(x), Node::Num(y)) if x.is_nan() && y.is_nan())
                })
                && a.lits == b.lits
                && a.function_of_y == b.function_of_y
        }
        _ => a == b,
    }
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
                    angle_style: d.angle_style,
                    function_of_y: e.function_of_y,
                },
            })
        }
        def @ (Def::Number(_) | Def::Angle(_)) => Some(Expression {
            kind: def.kind(),
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
    let (nodes, lits, root, angle_style, function_of_y) = match &e.form {
        Form::Tree {
            nodes,
            lits,
            root,
            angle_style,
            function_of_y,
        } => (nodes, lits, *root, *angle_style, *function_of_y),
        Form::Exact { def, dec } => {
            let id = c.add(name, *def)?;
            set_free_decimal(c, id, dec.clone());
            return Ok(id);
        }
    };
    let parsed = reparse(c, e.kind, nodes, lits, root).ok_or(Error::UnknownObject)?;
    let fits = matches!(
        (e.kind, parsed.tys[parsed.root as usize]),
        (
            Kind::Function | Kind::FunctionNVar | Kind::Number | Kind::Angle,
            Ty::Num(_)
        ) | (Kind::Boolean | Kind::Function, Ty::Bool)
            | (Kind::Point | Kind::Vector, Ty::Vec)
            | (Kind::List, Ty::List(_))
            | (Kind::Text, Ty::Text)
    );
    if !fits {
        return Err(Error::UnknownObject);
    }
    add_parsed(c, name, parsed, e.kind, angle_style, function_of_y)
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
            Node::Obj(id) | Node::ObjectDefined(id) | Node::FunctionObject(id) => {
                (id.0 < c.len()).then_some(())?;
                read(id, true);
            }
            Node::ItemObject(id) => {
                (id.0 < c.len() && list::object_kind(c.kind(id))).then_some(())?;
                read(id, true);
                for p in list::object_parents(c, id) {
                    read(p, false);
                }
            }
            Node::Relation {
                objects,
                count,
                list,
            } => {
                if let Some(list) = list {
                    (list.0 < c.len() && c.kind(list) == Kind::List).then_some(())?;
                    read(list, true);
                }
                for &id in objects.get(..usize::from(count))? {
                    (id.0 < c.len()).then_some(())?;
                    read(id, true);
                }
            }
            Node::Side(eq, left) => {
                (eq.0 < c.len()
                    && matches!(
                        c.kind(eq),
                        Kind::Line | Kind::Circle | Kind::Conic | Kind::ImplicitCurve
                    ))
                .then_some(())?;
                read(eq, true);
                for &id in equation::side_parents(c, eq, left) {
                    read(id, true);
                }
            }
            Node::Apply2(f, ..) => {
                (f.0 < c.len() && c.kind(f) == Kind::FunctionNVar).then_some(())?;
                read(f, true);
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
    if uses_var && kind != Kind::Function && vars[r] != 0
        || kind == Kind::Function && reads_list(&nodes, &tys)
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
        uses_y: false,
        depth: depth[r],
        cost: cost[r],
        vars: vars[r],
    })
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

pub(crate) fn undefined_literal(c: &Construction, d: ExprDef) -> bool {
    let e = entry(c, d);
    matches!(e.nodes[e.root as usize], Node::Num(v) if v.is_nan())
}

/// The objects an expression object reads.
pub(crate) fn parents(c: &Construction, def: &Def) -> Vec<Id> {
    match *def {
        Def::Expr(d) => entry(c, d).parents.clone(),
        _ => Vec::new(),
    }
}

/// An operand of a comparison as the symbolic prover reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operand {
    /// A named object.
    Object(Id),
    /// A segment between two points: a distance of two points, a segment
    /// object, or the segment the prover put in place of a distance.
    Segment([Id; 2]),
    /// A constant integer.
    Integer(i64),
    /// Anything else.
    Other,
}

/// The comparison at the root of a Boolean expression and its operands.
pub(crate) fn comparison(c: &Construction, boolean: Id) -> Option<(Bin, [Operand; 2])> {
    let Def::Expr(def) = c.def(boolean) else {
        return None;
    };
    let expr = entry(c, def);
    let Node::Bin(op, lhs, rhs) = expr.nodes[expr.root as usize] else {
        return None;
    };
    Some((op, [lhs, rhs].map(|side| operand(c, expr, side))))
}

/// The operand at node `index` of `expr`.
pub(crate) fn operand(c: &Construction, expr: &Entry, index: u32) -> Operand {
    if let Some(&(_, pair)) = expr.segment_operands.iter().find(|(n, _)| *n == index) {
        return Operand::Segment(pair);
    }
    let points = |a: Id, b: Id| c.kind(a) == Kind::Point && c.kind(b) == Kind::Point;
    match expr.nodes[index as usize] {
        Node::Obj(id) => match c.def(id) {
            Def::Distance(a, b) | Def::Segment(a, b) if points(a, b) => Operand::Segment([a, b]),
            _ => Operand::Object(id),
        },
        Node::Cmd(Def::Distance(a, b), _) if points(a, b) => Operand::Segment([a, b]),
        Node::Num(v) if v.fract() == 0.0 && v.abs() < 1e15 => Operand::Integer(v as i64),
        _ => Operand::Other,
    }
}

/// The node tree of a Boolean expression, for the prover's translation of
/// arithmetic statements.
pub(crate) fn boolean_entry(c: &Construction, boolean: Id) -> Option<&Entry> {
    match c.def(boolean) {
        Def::Expr(def) => Some(entry(c, def)),
        _ => None,
    }
}

/// What the symbolic prover does to a Boolean it translates
/// (`DependentBooleanAdapter.getBotanaPolynomials`): each operand of the
/// root that is a distance of two points becomes a segment, an existing one
/// between the same points if the construction has one, otherwise a new one
/// whose length is never computed and stays 0. The change is permanent:
/// later updates of the Boolean compare those segments. Its parents stay
/// the same, so it is updated when the distances are.
pub(crate) fn replace_distance_operands(c: &mut Construction, boolean: Id) -> bool {
    let Def::Expr(def) = c.def(boolean) else {
        return false;
    };
    let expr = entry(c, def);
    let Node::Bin(_, lhs, rhs) = expr.nodes[expr.root as usize] else {
        return false;
    };
    let mut replaced = Vec::new();
    for side in [lhs, rhs] {
        let pair = match expr.nodes[side as usize] {
            Node::Obj(id) => match c.def(id) {
                Def::Distance(a, b) => [a, b],
                _ => continue,
            },
            Node::Cmd(Def::Distance(a, b), _) => [a, b],
            _ => continue,
        };
        if pair.iter().any(|&p| c.kind(p) != Kind::Point) {
            continue;
        }
        let existing = (0..c.len()).map(Id).find(|&id| {
            c.exists(id)
                && matches!(c.def(id), Def::Segment(a, b)
                    if [a, b] == pair || [b, a] == pair)
        });
        let node = existing.map_or(Node::Num(0.0), Node::Obj);
        replaced.push((side, pair, node));
    }
    let changed = !replaced.is_empty();
    let e = &mut c.exprs.entries[def.index as usize];
    for (side, pair, node) in replaced {
        e.nodes[side as usize] = node;
        e.segment_operands.push((side, pair));
    }
    changed
}

/// A result of [`evaluate`]: the raw value and the exact decimal of a
/// number.
pub(crate) struct Outcome {
    pub(crate) raw: Raw,
    dec: Option<eval::Exact>,
}

/// Evaluates an expression object without lists for the current values of
/// its parents.
pub(crate) fn evaluate(c: &Construction, d: ExprDef) -> Outcome {
    let e = entry(c, d);
    if e.function {
        // a function that is the undefined constant alone is undefined
        // (`f(x) = ?`), any other one with undefined values is defined
        let raw = Raw::Num(if tree_defined(c, e) { 0.0 } else { f64::NAN });
        Outcome { raw, dec: None }
    } else {
        let (raw, dec) = eval::root(c, e, None);
        Outcome { raw, dec }
    }
}

/// Evaluates an expression object with lists, which builds them in the
/// scratch `lists`; the elements of a list result go to `out`.
fn evaluate_lists(
    c: &Construction,
    d: ExprDef,
    lists: &eval::Lists,
    out: &mut Vec<Slot>,
) -> Outcome {
    let e = entry(c, d);
    let raw = if let Ty::List(_) = e.root_ty() {
        match eval::root_list(c, e, lists, out) {
            Some(len) => Raw::List(len),
            None => {
                out.clear();
                Raw::Num(f64::NAN)
            }
        }
    } else {
        let (raw, dec) = eval::root(c, e, Some(lists));
        return Outcome { raw, dec };
    };
    Outcome { raw, dec: None }
}

/// The first evaluation can reject an invalid list transform argument.
pub(crate) fn initialize(c: &mut Construction, id: Id, d: ExprDef) -> Result<Value, Error> {
    let e = entry(c, d);
    if e.lists
        && e.nodes.iter().any(|node| matches!(node, Node::If(..)))
        && e.nodes.iter().any(|node| matches!(node, Node::Dilate(..)))
    {
        let meter = eval::Lists::new(Vec::new());
        return eval::with_command_budget(Some(&meter), || {
            let transforms = eval::validate_dilations(c, entry(c, d), &meter)?;
            c.exprs.entries[d.index as usize].nested_transforms = transforms;
            initialize_value(c, id, d)
        });
    }
    initialize_value(c, id, d)
}

fn initialize_value(c: &mut Construction, id: Id, d: ExprDef) -> Result<Value, Error> {
    let (value, invalid_transform_list) = if entry(c, d).lists
        && !matches!(d.kind, Kind::Text | Kind::Function | Kind::FunctionNVar)
    {
        let ((value, _), invalid) = update_lists(c, d);
        (value, invalid)
    } else {
        let (value, _) = update(c, id, d);
        let invalid = matches!(&entry(c, d).extra, StoredExtra::Text(result)
            if result.invalid_transform_list);
        (value, invalid)
    };
    if invalid_transform_list {
        return Err(Error::InvalidTransformList);
    }
    Ok(value)
}

/// The stored result is needed for rollback after the work deadline has
/// passed, when evaluating the expression again would have no allowance.
pub(crate) struct SavedResult {
    raw: Raw,
    extra: StoredExtra,
    interval: Option<FunctionInterval>,
    items: Vec<Slot>,
    coordinate_layout: Option<list::CoordinateLayout>,
    stuck: Vec<u32>,
    list_records: Option<ListRecords>,
    nested_transforms: NestedTransforms,
}

impl SavedResult {
    pub(crate) fn of(c: &Construction, d: ExprDef) -> Option<Self> {
        let e = entry(c, d);
        let extra_work = match &e.extra {
            StoredExtra::Text(text) => text.text.as_ref().map_or(0, String::len),
            StoredExtra::Polynomial(coefficients) => coefficients.len(),
            _ => 0,
        };
        let stuck = e.stuck.nodes();
        let list_records = e.lists.then(|| e.list_history.records());
        let record_work = list_records.as_ref().map_or(0, |records| {
            records.shapes.len() + records.invalid_transposes.len()
        });
        // Pay for saving, and for dropping or restoring the saved slots.
        let transform_work = e
            .nested_transforms
            .values()
            .map(|(_, slots)| 1 + slots.len())
            .sum::<usize>();
        let work = 2
            * (1 + e.items.len() + extra_work + stuck.len() + transform_work + record_work) as u64;
        reserve_work(work).then(|| Self {
            raw: e.raw,
            extra: e.extra.clone(),
            interval: e.interval,
            items: e.items.clone(),
            coordinate_layout: e.coordinate_layout,
            stuck: stuck.clone(),
            list_records: list_records.as_deref().cloned(),
            nested_transforms: e.nested_transforms.clone(),
        })
    }

    pub(crate) fn restore(self, c: &mut Construction, d: ExprDef) {
        let e = &mut c.exprs.entries[d.index as usize];
        e.raw = self.raw;
        e.extra = self.extra;
        e.interval = self.interval;
        e.items = self.items;
        e.nested_transforms = self.nested_transforms;
        e.coordinate_layout = self.coordinate_layout;
        *e.stuck.nodes() = self.stuck;
        if let Some(records) = self.list_records {
            *e.list_history.records() = records;
        }
    }
}

/// Recomputes the expression object `d` and keeps its result; its value,
/// and whether it changed (see [`store`]).
pub(crate) fn update(c: &mut Construction, id: Id, d: ExprDef) -> (Value, bool) {
    let e = entry(c, d);
    if matches!(e.nodes[e.root as usize], Node::FunctionAsymptotes(a) if matches!(e.nodes[a as usize], Node::Obj(_)))
        && c.exists(id)
        && c.value(id) == Value::Undefined
    {
        // The implicit asymptote algorithm leaves its list undefined once
        // its input becomes undefined, even if it later computes new lines.
        return (Value::Undefined, false);
    }
    if let (Kind::Function | Kind::FunctionNVar, Some((_, op, a, b))) = (d.kind, e.cas_root()) {
        let result = cas::function(c, e, op, a, b);
        let e = &mut c.exprs.entries[d.index as usize];
        e.raw = Raw::Num(if result.is_some() { 0.0 } else { f64::NAN });
        e.extra = result.map_or(StoredExtra::None, |form| {
            StoredExtra::Cas(std::sync::Arc::new(form))
        });
        let value = if d.kind == Kind::FunctionNVar && matches!(e.extra, StoredExtra::Cas(_)) {
            Value::FunctionNVar
        } else {
            value(d, e.raw)
        };
        return (value, true);
    }

    if d.kind == Kind::FunctionNVar {
        return (current(c, d), true);
    }
    if d.kind == Kind::Text {
        let (result, transforms) = eval::text_root(c, entry(c, d), c.text_latex_override(id));
        let e = &mut c.exprs.entries[d.index as usize];
        e.nested_transforms.extend(transforms);
        let old = match &e.extra {
            StoredExtra::Text(old) => Some(old.as_ref()),
            _ => None,
        };
        let changed = Some(&result) != old;
        if changed {
            e.extra = StoredExtra::Text(Box::new(result));
        }
        return (
            if e.text().is_some() {
                Value::Text
            } else {
                Value::Undefined
            },
            changed,
        );
    }
    if entry(c, d).lists {
        return update_lists(c, d).0;
    }
    if let Some(interval) = entry(c, d).evaluate_interval(c) {
        c.exprs.entries[d.index as usize].interval = Some(interval);
    }
    let e = entry(c, d);
    if let Node::PolynomialPart(remainder, a, b) = e.nodes[e.root as usize] {
        let coefficients = commands::polynomial::divide(c, e, remainder, a, b);
        let e = &mut c.exprs.entries[d.index as usize];
        e.raw = Raw::Num(if coefficients.is_some() {
            0.0
        } else {
            f64::NAN
        });
        e.extra = coefficients.map_or(StoredExtra::None, StoredExtra::Polynomial);
        return (value(d, e.raw), true);
    }
    let out = evaluate(c, d);
    (value(d, out.raw), store(c, d, out))
}

/// [`update`] of a tree with lists. The scratch buffers are taken from the
/// table and given back, so a recompute allocates nothing once they have
/// grown.
#[inline(never)]
fn update_lists(c: &mut Construction, d: ExprDef) -> ((Value, bool), bool) {
    let lists = eval::Lists::new(std::mem::take(&mut c.exprs.arena));
    let mut items = std::mem::take(&mut c.exprs.out);
    items.clear();
    let out = evaluate_lists(c, d, &lists, &mut items);
    let invalid_transform_list = lists.invalid_transform_list.get();
    let transforms = lists.take_nested_transforms();
    c.exprs.arena = lists.into_arena();
    let e = &mut c.exprs.entries[d.index as usize];
    e.nested_transforms.extend(transforms);
    let items_changed = !list::same_slots(&e.items, &items);
    if items_changed {
        e.items.clear();
        e.items.extend_from_slice(&items);
    }
    if items_changed || !e.raw.same(out.raw) {
        e.coordinate_layout = match out.raw {
            Raw::List(len) => list::CoordinateLayout::of(List::new(&items, len)),
            _ => None,
        };
    }
    c.exprs.out = items;
    let v = value(d, out.raw);
    let changed = store(c, d, out);
    ((v, changed || items_changed), invalid_transform_list)
}

/// `AlgoDependentFunction.inputDefined`: a function is defined when every
/// object it names is, and every command in it has a result. A number with
/// an infinite raw value counts as defined, as in GeoGebra.
fn inputs_defined(c: &Construction, e: &Entry) -> bool {
    if e.interval.is_some_and(|interval| !interval.defined) {
        return false;
    }
    if e.checks_branches {
        let meter = eval::Lists::new(Vec::new());
        let ctx = eval::Ctx::<true> {
            meter: Some(&meter),
            ..eval::Ctx::new(c, e, &eval::Env::None)
        };
        return defined_below(&ctx, e.root) && !meter.over();
    }
    e.inputs.iter().all(|&id| function_input_defined(c, id))
        && e.nodes.iter().all(|n| match *n {
            Node::Cmd(def, hint) => command_defined(c, def, hint),
            _ => true,
        })
}

pub(crate) fn object_defined(c: &Construction, id: Id) -> bool {
    match (c.kind(id), c.def(id)) {
        (Kind::Number | Kind::Angle, Def::Expr(d)) => {
            !matches!(raw(c, d), Raw::Num(v) if v.is_nan())
        }
        (Kind::Circle, _) => c.value(id).is_defined() || c.has_infinite_circle_center(id),
        _ => c.value(id).is_defined(),
    }
}

fn function_input_defined(c: &Construction, id: Id) -> bool {
    // A projective point is defined in GeoGebra. Its Cartesian coordinates
    // still evaluate to NaN, and commands that require them stay undefined.
    object_defined(c, id)
        || c.point_at_infinity(id)
            .is_some_and(|(x, y, _)| x != 0.0 || y != 0.0)
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
fn defined_below(ctx: &eval::Ctx<'_, true>, i: u32) -> bool {
    if ctx.stop(1) {
        return false;
    }
    match ctx.node(i) {
        Node::Obj(id) => function_input_defined(ctx.c, id),
        Node::Apply(f, a) => object_defined(ctx.c, f) && defined_below(ctx, a),
        Node::Cmd(def, hint) => {
            eval::with_command_budget(ctx.meter, || command_defined(ctx.c, def, hint))
        }
        // CmdLength supplies a numeric helper for a point or vector.
        Node::Un(Un::Length, a) => defined_below(ctx, a) && !ctx.full_number(i).is_nan(),
        Node::Inverse(a) => defined_below(ctx, a) && inverse::sample(ctx, a, 1.0).is_some(),
        Node::Cas(op, a, b) if op.is_function() => cas::function_at(ctx, op, a, b).is_some(),
        Node::PolynomialPart(remainder, a, b) => {
            commands::polynomial::divide(ctx.c, ctx.e, remainder, a, b).is_some()
        }
        Node::Dilate(source, factor, _) => {
            defined_below(ctx, source)
                && !crate::algo::transform::zero_dilation(ctx.full_number(factor))
        }
        Node::If(IfMode::Algo | IfMode::AlgoNext, ..) => ctx
            .if_alternative(i)
            .is_some_and(|branch| defined_below(ctx, branch)),
        Node::If(..) => true,
        node => children(&node).all(|child| defined_below(ctx, child)),
    }
}

/// The value of an expression object for the current values of its
/// parents, without storing it.
#[inline(never)]
pub(crate) fn current(c: &Construction, d: ExprDef) -> Value {
    if d.kind == Kind::FunctionNVar {
        let e = entry(c, d);
        let defined = if let Some((_, op, a, b)) = e.cas_root() {
            cas::function(c, e, op, a, b).is_some()
        } else {
            match e.nodes[e.root as usize] {
                Node::Side(eq, _) => c.value(eq).is_defined(),
                Node::SystemFunction(_, root) => {
                    inputs_defined(c, e)
                        && !matches!(e.nodes[root as usize], Node::Num(v) if v.is_nan())
                }
                _ => false,
            }
        };
        return if defined {
            Value::FunctionNVar
        } else {
            Value::Undefined
        };
    }
    if d.kind == Kind::Text {
        return if eval::text_root(c, entry(c, d), None).0.text.is_some() {
            Value::Text
        } else {
            Value::Undefined
        };
    }
    let raw = if entry(c, d).lists {
        evaluate_lists(c, d, &eval::Lists::new(Vec::new()), &mut Vec::new()).raw
    } else {
        evaluate(c, d).raw
    };
    value(d, raw)
}

fn kept_list_shape(c: &Construction, id: Id) -> Option<std::sync::Arc<[Kind]>> {
    let Def::Expr(d) = c.def(id) else {
        return None;
    };
    let e = entry(c, d);
    e.kept_list_shape(e.root)
}

/// The elements of the list object `id`, when it is a defined list.
pub(crate) fn list(c: &Construction, id: Id) -> Option<List<'_>> {
    let Value::List { len } = c.value(id) else {
        return None;
    };
    Some(List::new(list_slots(c, id)?, len))
}

fn list_slots(c: &Construction, id: Id) -> Option<&[Slot]> {
    match c.def(id) {
        Def::Expr(d) => Some(&entry(c, d).items),
        Def::Chart(_) => c.chart(id).map(|data| data.slots.as_slice()),
        Def::Prove(_) => c.proof(id).map(|data| data.slots.as_slice()),
        _ => None,
    }
}

pub(crate) fn coordinate_rows(c: &Construction, id: Id) -> Option<list::CoordinateRows<'_>> {
    let (Def::Expr(d), Value::List { len }) = (c.def(id), c.value(id)) else {
        return None;
    };
    let e = entry(c, d);
    Some(list::CoordinateRows::new(
        List::new(&e.items, len),
        e.coordinate_layout?,
    ))
}

/// Stores the result of [`evaluate`]; tells whether it changed. A function
/// always counts as changed: its values depend on its inputs even when it
/// stays defined.
/// Keeps a result; whether it changed (a decimal of the same value counts:
/// it changes what reads the number).
pub(crate) fn store(c: &mut Construction, d: ExprDef, out: Outcome) -> bool {
    let e = &mut c.exprs.entries[d.index as usize];
    let extra = StoredExtra::from(out.dec);
    let changed = e.function || !e.raw.same(out.raw) || e.extra != extra;
    e.raw = out.raw;
    e.extra = extra;
    changed
}

/// `GeoFunction.value`: the value of the function `f` at `x`, `None` when
/// `f` is not a defined function.
pub(crate) fn function_value(c: &Construction, f: Id, x: f64) -> Option<f64> {
    match (c.def(f), c.value(f)) {
        (Def::Expr(d), Value::Function) => Some(eval::sample(c, entry(c, d), x)),
        (Def::Calculus(d), Value::Function) => {
            let record = c.calculus_record(f)?;
            d.record_value(c, record, f, x)
        }
        (Def::Chart(_), Value::Function) => c.chart(f).map(|data| data.sample(x)),
        _ => None,
    }
}

/// Whether the function object `f` has no expression tree and answers by
/// sampling: a chart function or a freehand function of a list.
pub(crate) fn is_sampled_function(c: &Construction, f: Id) -> bool {
    matches!(
        c.def(f),
        Def::Chart(_) | Def::Calculus(crate::algo::calculus::CalculusDef::Freehand { .. })
    )
}

/// The attributes that must agree before comparing two univariate functions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionSignature {
    pub(crate) boolean: bool,
    pub(crate) of_y: bool,
}

pub(crate) fn function_signature(c: &Construction, id: Id) -> Option<FunctionSignature> {
    if c.kind(id) == Kind::Function && matches!(c.def(id), Def::Chart(_)) {
        return Some(FunctionSignature {
            boolean: false,
            of_y: false,
        });
    }
    let Def::Expr(d) = c.def(id) else {
        return None;
    };
    let e = entry(c, d);
    e.function.then(|| FunctionSignature {
        boolean: e.root_ty() == Ty::Bool,
        of_y: e.function_of_y,
    })
}

pub(crate) use commands::logical::{conditional_branch, conditional_kind, define_geometry_if};
pub(crate) use commands::relation::{define_relation, relation_reports};

pub(crate) fn is_independent_text(c: &Construction, id: Id) -> bool {
    commands::text::is_independent(c, id)
}

pub(crate) fn text_presentation(
    c: &Construction,
    id: Id,
) -> Option<crate::kernel::text::TextPresentation> {
    let Def::Expr(def) = c.def(id) else {
        return None;
    };
    match &entry(c, def).extra {
        StoredExtra::Text(result) => Some(result.presentation),
        _ => None,
    }
}

pub(crate) fn accepts_text_argument(c: &Construction, source: &str) -> bool {
    parse::parse_argument(c, source).is_ok()
}

pub(crate) use cas::displayed as symbolic_value;
pub(crate) use commands::text::refuse_empty_simplify;

pub(crate) fn text_value(c: &Construction, id: Id) -> Option<&str> {
    let Def::Expr(d) = c.def(id) else {
        return None;
    };
    entry(c, d).text()
}

pub(crate) fn relation_number(c: &Construction, id: Id) -> f64 {
    match c.value(id) {
        Value::Number(v) => v,
        _ => match c.def(id) {
            Def::Expr(d) if c.kind(id).is_number() => match raw(c, d) {
                Raw::Num(v) => v,
                _ => f64::NAN,
            },
            _ => f64::NAN,
        },
    }
}

/// The entry of a tree the calculus commands built themselves (see
/// `algo::calculus`), sampled like a function's tree.
pub(crate) fn function_entry(c: &Construction, parsed: parse::Parsed) -> Entry {
    Entry::new(c, parsed, true)
}

/// The entry of the expression object `d`, for the calculus commands that
/// read the tree of their source function.
pub(crate) fn source_entry(c: &Construction, d: ExprDef) -> &Entry {
    entry(c, d)
}

/// `GeoFunction.value` of a tree the calculus commands built, without
/// keeping the entry (the antiderivative at a bound of integration).
pub(crate) fn sample_parsed(c: &Construction, parsed: &parse::Parsed, x: f64) -> f64 {
    eval::sample(c, &Entry::new(c, parsed.clone(), true), x)
}

/// `GeoFunction.value` of a tree the calculus commands built; the entry is
/// one of their records.
pub(crate) fn sample_tree(c: &Construction, e: &Entry, x: f64) -> f64 {
    eval::sample(c, e, x)
}

/// `GeoFunction.value` of the subtree `i` of a function's tree: a branch
/// of a conditional function the conditional integral integrates
/// (`AlgoIntegralDefinite.computeConditional`).
pub(crate) fn sample_subtree(c: &Construction, e: &Entry, i: u32, x: f64) -> f64 {
    eval::sample_subtree(c, e, i, x)
}

/// The boolean value of the subtree `i` of a function's tree at `x`, and
/// whether it is defined (`Bounds.addRestriction`'s condition checks).
pub(crate) fn subtree_bool(c: &Construction, e: &Entry, i: u32, x: f64) -> Option<(bool, bool)> {
    match eval::subtree(c, e, i, x) {
        eval::Val::B(v, defined) => Some((v, defined)),
        _ => None,
    }
}

/// Whether the subtree `i` of a function's tree is a number (`Ty::Num`),
/// which the constant side of a branch bound's comparison must be
/// (`NumberValue`).
pub(crate) fn subtree_is_number(e: &Entry, i: u32) -> bool {
    matches!(e.tys[i as usize], Ty::Num(_))
}

/// `GeoFunction.isDefined` of a tree the calculus commands built: its
/// objects are defined, and the root is not the constant `NaN` that
/// GeoGebra's no-CAS integrator builds for a rule it cannot integrate
/// (`ExpressionNode.isDefined`).
pub(crate) fn tree_defined(c: &Construction, e: &Entry) -> bool {
    inputs_defined(c, e) && !matches!(e.nodes[e.root as usize], Node::Num(v) if v.is_nan())
}

/// The nodes of an entry, for the calculus commands that walk the tree of
/// their source function.
pub(crate) fn nodes_of(e: &Entry) -> &[Node] {
    &e.nodes
}

/// The root node index of an entry.
pub(crate) fn root_of(e: &Entry) -> u32 {
    e.root
}

/// The number literals of a tree, in tree order
/// (`AlgoCoefficients.extractConstants`: every `MyDouble` leaf is a
/// constant — a degree sign included — collected by the pre-order walk of
/// `ExpressionNode.any`, which covers the tree, not the parser's arena).
fn constant_literals(e: &Entry, root: u32) -> Vec<f64> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(i) = stack.pop() {
        match e.nodes[i as usize] {
            Node::Num(v) => out.push(v),
            Node::MinusOne => out.push(-1.0),
            Node::Degree => out.push(math::PI_180),
            ref node => {
                let mut kids: Vec<u32> = children(node).filter(|&k| k != NONE).collect();
                kids.reverse();
                stack.extend(kids);
            }
        }
    }
    out
}

/// The parsed tree of an entry again, for the copy an order-zero
/// derivative is.
/// The parameter name a Cartesian curve's coordinate function uses, for
/// the variable forms of the derivative commands.
pub(crate) fn curve_parameter(e: &Entry) -> Option<&str> {
    e.curve_parameter.as_deref()
}

pub(crate) fn parsed_of(e: &Entry) -> parse::Parsed {
    parse::Parsed {
        root: e.root,
        nodes: e.nodes.clone(),
        lits: e.lits.clone(),
        tys: e.tys.clone(),
        parents: e.parents.clone(),
        inputs: e.inputs.clone(),
        depth: e.depth,
        cost: e.cost,
        vars: e.vars,
        uses_var: false,
        uses_y: false,
    }
}

/// Charges `n` units of evaluation work (see [`eval`]), which the loops of
/// the calculus commands stay inside.
pub(crate) fn charge(n: u64) {
    eval::work(n);
}

/// Adds a tree the calculus commands built as the object `name`: a
/// coefficient list, or a hidden list of points. The kinds are those
/// [`add_parsed`] takes; a tree without parents still becomes a
/// dependent object, like a list it builds.
pub(crate) fn add_built(
    c: &mut Construction,
    name: &str,
    parsed: parse::Parsed,
    kind: Kind,
) -> Result<Id, Error> {
    add_parsed(c, name, parsed, kind, AngleStyle::Unbounded, false)
}

/// Whether the expression is `Product(expression, k, from, to)` of a point
/// or vector expression.
pub(crate) fn is_coordinate_product(c: &Construction, d: ExprDef) -> bool {
    let e = entry(c, d);
    matches!(e.nodes[e.root as usize], Node::Call(CallOp::Fold, ..))
        && e.tys[e.root as usize] == Ty::Vec
}

/// The last stored result.
pub(crate) fn raw(c: &Construction, d: ExprDef) -> Raw {
    entry(c, d).raw
}

/// The value an object shows for a result. An infinite number is a value
/// like any other (`GeoNumeric.isDefined` only refuses NaN). A dependent
/// point with an infinite coordinate is undefined (`AlgoDependentPoint`);
/// an angle or a vector keeps its infinity in [`Raw`] but shows as
/// undefined.
pub(crate) fn value(d: ExprDef, raw: Raw) -> Value {
    match (d.kind, raw) {
        (Kind::Number, Raw::Num(v)) if !v.is_nan() => Value::Number(v),
        (Kind::Angle, Raw::Num(v)) if v.is_finite() => Value::Number(d.angle_style.value(v)),
        // AlgoDependentPoint rejects infinite coordinates. A constant point,
        // created directly by AlgebraProcessor, still owns both coordinates.
        (Kind::Point, Raw::Vec(x, y))
            if (x.is_finite() && y.is_finite())
                || (d.allows_infinite_point && !(x.is_nan() || y.is_nan())) =>
        {
            Value::Point { x, y }
        }
        (Kind::Point, Raw::Homogeneous([x, y, z])) => Value::homogeneous_point(x, y, z),
        (Kind::Vector, Raw::Vec(x, y)) if !(x.is_nan() || y.is_nan()) => Value::Vector { x, y },
        (Kind::Function, Raw::Num(v)) if !v.is_nan() => Value::Function,
        // `AlgoDependentBoolean`: a boolean is never undefined, a
        // comparison of an undefined number is false
        (Kind::Boolean, Raw::Num(v)) if !v.is_nan() => Value::Bool(v == 1.0),
        (Kind::List, Raw::List(len)) => Value::List { len },
        _ => Value::Undefined,
    }
}

/// Adds a live dependency for an inline command argument of the given kind.
pub(crate) fn argument_object(c: &mut Construction, text: &str, label: &str, kind: Kind) -> bool {
    let Ok(parsed) = parse::parse_argument(c, text) else {
        return false;
    };
    let kind = match (kind, parsed.tys[parsed.root as usize]) {
        (Kind::Number, Ty::Num(1)) => Kind::Angle,
        (Kind::Number, Ty::Num(_))
        | (Kind::List, Ty::List(_))
        | (Kind::Boolean, Ty::Bool)
        | (Kind::Point, Ty::Vec) => kind,
        _ => return false,
    };
    !parsed.uses_var && add(c, label, parsed, kind).is_ok()
}

/// Builds a list from resolved arguments whose hidden labels cannot be written
/// in an expression.
pub(crate) fn list_of_objects(
    c: &mut Construction,
    label: &str,
    ids: Vec<Id>,
) -> Result<Id, String> {
    let count = u32::try_from(ids.len()).map_err(|_| "too many arguments")?;
    let root = count.checked_mul(2).ok_or("too many arguments")?;
    if u64::from(root) + 1 > parse::MAX_COST {
        return Err("too many arguments".into());
    }
    let mut nodes = Vec::with_capacity(root as usize + 1);
    nodes.extend(ids.iter().copied().map(Node::Obj));
    nodes.extend(ids.iter().enumerate().map(|(i, &id)| {
        if list::object_kind(c.kind(id)) {
            Node::ItemObject(id)
        } else {
            Node::Item(i as u32)
        }
    }));
    nodes.push(Node::List(count, count));
    let (tys, depth, cost, vars) = types::check(c, &nodes, root)?;
    let mut parents = ids.clone();
    parents.sort_unstable();
    parents.dedup();
    let parsed = parse::Parsed {
        lits: vec![Lit::default(); nodes.len()],
        nodes,
        tys,
        root,
        parents,
        inputs: ids,
        uses_var: false,
        uses_y: false,
        depth: depth[root as usize],
        cost: cost[root as usize],
        vars: vars[root as usize],
    };
    add(c, label, parsed, Kind::List)
}

/// Evaluates a self-contained scalar argument without adding a construction
/// object. The second result marks a value entered with an angle dimension.
pub(crate) fn constant_number(c: &Construction, text: &str) -> Option<(f64, bool)> {
    let parsed = parse::parse(c, text).ok()?;
    if parsed.uses_var || !parsed.parents.is_empty() {
        return None;
    }
    let entry = Entry::new(c, parsed, false);
    let (raw, _) = eval::root(c, &entry, None);
    match raw {
        Raw::Num(v) if v.is_finite() => Some((v, entry.root_ty() == Ty::Num(1))),
        _ => None,
    }
}

/// The current value of a parsed numeric tree, reading the current values of
/// its objects.
pub(crate) fn value_of_tree(c: &Construction, parsed: &parse::Parsed) -> Option<f64> {
    let entry = Entry::new(c, parsed.clone(), false);
    match eval::root(c, &entry, None).0 {
        Raw::Num(v) => Some(v),
        _ => None,
    }
}

/// Reads the current value of a numeric argument without adding an object.
pub(crate) fn argument_number(c: &Construction, text: &str) -> Option<f64> {
    let parsed = parse::parse_argument(c, text).ok()?;
    if parsed.uses_var || parsed.uses_y || !matches!(parsed.tys[parsed.root as usize], Ty::Num(_)) {
        return None;
    }
    value_of_tree(c, &parsed)
}

/// [`argument_number`] for arithmetic this evaluation runs in full, which
/// tells a tree it does not support from inputs that are undefined right
/// now. `None` when the text is no numeric argument over the construction,
/// or when its tree holds a node outside plain arithmetic: a list, a text,
/// a command computed in a list or by the computer algebra code, anything
/// [`argument_number`] leaves undefined whatever the inputs. Otherwise the
/// current value, `NaN` while an input is undefined.
pub(crate) fn arithmetic_number(c: &Construction, text: &str) -> Option<f64> {
    let parsed = parse::parse_argument(c, text).ok()?;
    let arithmetic = parsed.nodes.iter().all(|node| {
        matches!(
            node,
            Node::Num(_)
                | Node::MinusOne
                | Node::Degree
                | Node::Obj(_)
                | Node::Bool(_)
                | Node::Un(..)
                | Node::Bin(..)
                | Node::If(..)
                | Node::Cmd(..)
                | Node::Apply(..)
                | Node::Tuple(..)
                | Node::Polar(..)
                | Node::ObjectDefined(_)
                | Node::Defined(..)
        )
    });
    if !arithmetic
        || reads_list(&parsed.nodes, &parsed.tys)
        || parsed.uses_var
        || parsed.uses_y
        || !matches!(parsed.tys[parsed.root as usize], Ty::Num(_))
    {
        return None;
    }
    value_of_tree(c, &parsed)
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
    define_with_coordinates(c, name, rhs, Some(vector_label))
}

pub(crate) fn define_saved_argument(
    c: &mut Construction,
    name: &str,
    rhs: &str,
) -> Result<Id, String> {
    define_with_coordinates(c, name, rhs, None)
}

fn define_with_coordinates(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    vector_label: Option<bool>,
) -> Result<Id, String> {
    if let Some(result) = define_plane_inequality(c, name, rhs) {
        return result;
    }
    let parsed = match parse::parse(c, rhs) {
        Ok(parsed) => parsed,
        Err(error) => match parse::parse_equation(c, rhs) {
            // A labelled slope can use both coordinate variables, as an
            // inline argument to Locus or SlopeField already can.
            Ok(parsed)
                if parsed.uses_var
                    && parsed.uses_y
                    && matches!(parsed.tys[parsed.root as usize], Ty::Num(_)) =>
            {
                parsed
            }
            _ => return Err(error),
        },
    };
    if parsed.uses_var {
        if parsed.uses_y && matches!(parsed.nodes[parsed.root as usize], Node::Cas(..)) {
            return add(c, name, parsed, Kind::FunctionNVar);
        }
        return function(c, name, parsed, false);
    }
    // `Sum` or `Product` of a point or vector expression over a range
    // (`AlgoFoldExpression`): the result has the class of the expression,
    // whatever the label is.
    let vector_label = match parsed.nodes[parsed.root as usize] {
        Node::ListOp(ListOp::ApplyMatrixCoordinates { vector }, ..) => Some(vector),
        Node::Call(CallOp::Fold | CallOp::SumFold, first, _) => {
            Some(folds_to_vector(c, &parsed, first))
        }
        _ => vector_label,
    };
    let kind = match parsed.tys[parsed.root as usize] {
        Ty::Num(1) => Kind::Angle,
        Ty::Num(_) => Kind::Number,
        Ty::Vec if vector_label.unwrap_or_else(|| argument_is_vector(c, &parsed)) => Kind::Vector,
        Ty::Vec => Kind::Point,
        Ty::Bool => Kind::Boolean,
        Ty::List(_) => Kind::List,
        Ty::Object => return Err("this object cannot be assigned as an expression".into()),
        Ty::Text => Kind::Text,
        Ty::Function => return Err("a function needs an argument".into()),
    };
    add(c, name, parsed, kind)
}

pub(crate) fn define_plane_inequality(
    c: &mut Construction,
    label: &str,
    rhs: &str,
) -> Option<Result<Id, String>> {
    if !rhs.contains(['<', '>']) || !rhs.contains('y') {
        return None;
    }
    let mut parsed = parse::parse_equation(c, rhs).ok()?;
    if !parsed.uses_y || parsed.tys[parsed.root as usize] != Ty::Bool {
        return None;
    }
    parsed.nodes.push(Node::SystemFunction(2, parsed.root));
    parsed.lits.push(Lit::default());
    parsed.tys.push(Ty::Bool);
    parsed.root = parsed.nodes.len() as u32 - 1;
    Some(add(c, label, parsed, Kind::FunctionNVar))
}

/// ExpressionNode.evaluatesToVectorNotPoint, for an unlabelled 2D input.
fn argument_is_vector(c: &Construction, parsed: &parse::Parsed) -> bool {
    argument_is_vector_at(c, &parsed.nodes, &parsed.tys, parsed.root)
}

fn argument_is_vector_at(c: &Construction, nodes: &[Node], types: &[Ty], root: u32) -> bool {
    let mut vectors = Vec::with_capacity(root as usize + 1);
    for (node, ty) in nodes.iter().zip(types).take(root as usize + 1) {
        let read = |index: u32| vectors[index as usize];
        let vector = matches!(ty, Ty::Vec)
            && match *node {
                Node::Obj(id) => c.kind(id) == Kind::Vector,
                Node::Cmd(def, _) => def.kind() == Kind::Vector,
                Node::Vector(_) => true,
                Node::ListOp(ListOp::ApplyMatrixCoordinates { vector }, ..) => vector,
                Node::Bin(Bin::Add, a, b) if types[a as usize] == Ty::Vec => read(a) && read(b),
                Node::Bin(Bin::Add | Bin::Sub | Bin::Mul | Bin::Div, a, b) => read(a) || read(b),
                Node::If(_, _, value, _) => read(value),
                _ => false,
            };
        vectors.push(vector);
    }
    vectors[root as usize]
}

/// `ExpressionNode.shouldEvaluateToGeoVector` for the expression at `at`
/// that `Sum` or `Product` folds over a range: whether the terms, and with
/// them the result, are vectors and not points. The left operand decides
/// (a number or a vector object makes a vector, a coordinate pair or a
/// point does not), and the right operand can only take the answer back:
/// a point object does, a coordinate pair leaves it as it is.
fn folds_to_vector(c: &Construction, parsed: &parse::Parsed, at: u32) -> bool {
    let number = |id: Id| matches!(c.kind(id), Kind::Number | Kind::Angle);
    let node = parsed.nodes[at as usize];
    // As the left operand, or as the whole expression.
    let left = |n: u32| match parsed.nodes[n as usize] {
        Node::Num(_) | Node::MinusOne | Node::Degree | Node::Local(..) => true,
        Node::Obj(id) => c.kind(id) == Kind::Vector || number(id),
        Node::Cmd(def, _) => def.kind() == Kind::Vector,
        Node::Bin(..) | Node::Un(..) => folds_to_vector(c, parsed, n),
        _ => false,
    };
    match node {
        Node::Item(value) => folds_to_vector(c, parsed, value),
        Node::Obj(id) => c.kind(id) == Kind::Vector,
        Node::Un(_, a) => left(a),
        Node::Bin(_, a, b) => {
            left(a)
                && match parsed.nodes[b as usize] {
                    Node::Bin(..) | Node::Un(..) => folds_to_vector(c, parsed, b),
                    Node::Obj(id) => number(id) || c.kind(id) == Kind::Vector,
                    _ => true,
                }
        }
        Node::Cmd(def, _) => def.kind() == Kind::Vector,
        _ => false,
    }
}

/// Defines a coordinate expression with an explicit kind, regardless of
/// the label's case.
pub(crate) fn define_coordinates(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    vector: bool,
) -> Result<Id, String> {
    let parsed = parse::parse(c, rhs)?;
    if parsed.uses_var || parsed.tys[parsed.root as usize] != Ty::Vec {
        return Err("expected a point or vector expression".into());
    }
    add(
        c,
        name,
        parsed,
        if vector { Kind::Vector } else { Kind::Point },
    )
}

/// The names in an expression, using the same token boundaries as the parser.
pub fn expression_names(source: &str) -> Vec<&str> {
    lex::tokens(source).map_or_else(
        |_| Vec::new(),
        |tokens| {
            tokens
                .into_iter()
                .filter(|token| matches!(token.tok, lex::Tok::Name { .. }))
                .map(|token| &source[token.start..token.end])
                .collect()
        },
    )
}

/// The source expressions in a brace-delimited list literal.
pub fn list_literal_elements(source: &str) -> Option<Vec<&str>> {
    use lex::Tok;
    if !source.trim_start().starts_with('{') {
        return None;
    }
    let tokens = lex::tokens(source).ok()?;
    let (first, rest) = tokens.split_first()?;
    let (last, inner) = rest.split_last()?;
    if !matches!(first.tok, Tok::LBrace) || !matches!(last.tok, Tok::RBrace) {
        return None;
    }
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let (mut start, mut depth) = (first.end, 0usize);
    let mut elements = Vec::new();
    for token in inner {
        match token.tok {
            Tok::LParen | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBrace => depth = depth.checked_sub(1)?,
            Tok::Comma if depth == 0 => {
                elements.push(source[start..token.start].trim());
                start = token.end;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    elements.push(source[start..last.start].trim());
    Some(elements)
}

/// Defines a named function. A constant right side (`f(x) = 3`) is still
/// a function.
pub(crate) fn define_function(
    c: &mut Construction,
    name: &str,
    rhs: &str,
    of_y: bool,
) -> Result<Id, String> {
    let parsed = if of_y {
        parse::parse_parameter(c, rhs, "y")?
    } else {
        parse::parse(c, rhs)?
    };
    function(c, name, parsed, of_y)
}

pub(crate) fn define_slow_plot(
    c: &mut Construction,
    name: &str,
    source: Id,
    slider: Id,
    corners: [Id; 2],
) -> Result<Id, String> {
    let mut b = equation::Builder::empty();
    let [left, right] = corners.map(|id| {
        let corner = b.push(Node::Obj(id));
        b.push(Node::Un(Un::X, corner))
    });
    let width = b.push(Node::Bin(Bin::Sub, right, left));
    let fraction = b.push(Node::Obj(slider));
    let extent = b.push(Node::Bin(Bin::Mul, fraction, width));
    let cutoff = b.push(Node::Bin(Bin::Add, left, extent));
    let x = b.push(Node::Var);
    let condition = b.push(Node::Bin(Bin::Less, x, cutoff));
    let value = b.push(Node::Apply(source, x));
    let root = b.push(Node::If(IfMode::List, condition, value, NONE));
    function(c, name, b.finish(c, root)?, false)
}

pub(crate) fn define_system_function(
    c: &mut Construction,
    label: &str,
    rhs: &str,
    parameters: &[&str],
) -> Result<Id, String> {
    let parsed = parse::parse_system(c, rhs, parameters)?;
    add(c, label, parsed, Kind::FunctionNVar)
}

pub(crate) fn define_parametric_coordinate(
    c: &mut Construction,
    label: &str,
    rhs: &str,
    parameter: &str,
) -> Result<Id, String> {
    let id = numeric_function(c, label, parse::parse_parameter(c, rhs, parameter)?)?;
    let Def::Expr(def) = c.def(id) else {
        unreachable!("a curve coordinate is a function expression");
    };
    c.exprs.entries[def.index as usize].curve_parameter = Some(parameter.into());
    Ok(id)
}

/// Legacy Function stores the graph's bounds separately. Parse the bounds
/// with the body to retain all of their construction dependencies.
pub(crate) fn define_function_interval(
    c: &mut Construction,
    label: &str,
    restriction: &str,
    parameter: &str,
) -> Result<Id, String> {
    let parsed = parse::parse_parameter(c, restriction, parameter)?;
    let Node::If(_, condition, body, _) = parsed.nodes[parsed.root as usize] else {
        return Err("expected a function interval".into());
    };
    let Node::Bin(Bin::And, left, right) = parsed.nodes[condition as usize] else {
        return Err("expected two function bounds".into());
    };
    let (Node::Bin(Bin::LessEqual, min, _), Node::Bin(Bin::LessEqual, _, max)) =
        (parsed.nodes[left as usize], parsed.nodes[right as usize])
    else {
        return Err("expected two inclusive function bounds".into());
    };
    if !matches!(parsed.tys[body as usize], Ty::Num(_)) || reads_list(&parsed.nodes, &parsed.tys) {
        return Err("expected a numeric function on the interval".into());
    }
    let mut e = Entry::new(c, parsed, true);
    e.root = body;
    e.interval = Some(FunctionInterval {
        bounds: [min, max],
        range: None,
        defined: false,
    });
    e.interval = e.evaluate_interval(c);
    e.curve_parameter = Some(parameter.into());
    add_entry(c, label, e, Kind::Function, AngleStyle::Unbounded, false).map_err(|e| e.to_string())
}

/// A legacy function exposes its saved graph interval even while undefined.
pub(crate) fn function_interval(c: &Construction, id: Id) -> Option<Option<[f64; 2]>> {
    let Def::Expr(def) = c.def(id) else {
        return None;
    };
    Some(entry(c, def).interval?.range)
}

/// Whether the slope expression `text` reads `x` and whether it reads
/// `y`; `None` when it does not parse. The symbolic `SolveODE` forms
/// refuse a slope that reads neither (a number).
pub(crate) fn slope_variables(c: &Construction, text: &str) -> Option<(bool, bool)> {
    let parsed = parse::parse_equation(c, text).ok()?;
    Some((parsed.uses_var, parsed.uses_y))
}

/// Hidden two-variable function for a slope field or integral curve.
pub(crate) fn define_slope(c: &mut Construction, label: &str, rhs: &str) -> Result<Id, String> {
    numeric_function(c, label, parse::parse_equation(c, rhs)?)
}

fn numeric_function(c: &mut Construction, name: &str, parsed: parse::Parsed) -> Result<Id, String> {
    if parsed.tys[parsed.root as usize] == Ty::Bool {
        return Err("a function that gives a boolean is not supported here".into());
    }
    function(c, name, parsed, false)
}

fn function(
    c: &mut Construction,
    name: &str,
    parsed: parse::Parsed,
    of_y: bool,
) -> Result<Id, String> {
    match parsed.tys[parsed.root as usize] {
        Ty::Vec | Ty::Object | Ty::Function | Ty::Text => {
            return Err("a function must give a number or a boolean".into());
        }
        Ty::List(_) => return Err("a function cannot give a list".into()),
        Ty::Num(_) if of_y => return Err("a function of y must give a boolean".into()),
        Ty::Num(_) | Ty::Bool => {}
    }
    let root = parsed.root as usize;
    if parsed
        .nodes
        .iter()
        .enumerate()
        .any(|(i, node)| i != root && matches!(node, Node::PolynomialPart(..)))
    {
        return Err(
            "name the polynomial division result before using it in another function".into(),
        );
    }
    if reads_list(&parsed.nodes, &parsed.tys) {
        return Err("a list in a function is not supported".into());
    }
    add_parsed(c, name, parsed, Kind::Function, AngleStyle::Unbounded, of_y)
        .map_err(|e| e.to_string())
}

fn add(c: &mut Construction, name: &str, parsed: parse::Parsed, kind: Kind) -> Result<Id, String> {
    add_parsed(c, name, parsed, kind, AngleStyle::Unbounded, false).map_err(|e| e.to_string())
}

/// A hidden function object of the inline expression `text`, the argument
/// of a command that reads a function (`sin(x)` in `Polynomial(sin(x))`;
/// see `script::resolve_geometry_arguments`).
pub(crate) fn argument_function(c: &mut Construction, text: &str, label: &str) -> Option<Id> {
    let parsed = parse::parse(c, text).ok()?;
    match parsed.tys[parsed.root as usize] {
        Ty::Num(_) if parsed.uses_var => add(c, label, parsed, Kind::Function).ok(),
        _ => None,
    }
}

/// A number as the constant function it counts as where GeoGebra accepts
/// one (`isRealValuedFunction` of a `GeoNumeric`): the second curve of
/// `Integral` and `IntegralBetween` (`CmdIntegral`'s first four-argument
/// branch).
pub(crate) fn constant_function(c: &mut Construction, text: &str, label: &str) -> Option<Id> {
    let parsed = parse::parse(c, text).ok()?;
    match parsed.tys[parsed.root as usize] {
        Ty::Num(_) if !parsed.uses_var => add(c, label, parsed, Kind::Function).ok(),
        _ => None,
    }
}

fn add_parsed(
    c: &mut Construction,
    name: &str,
    parsed: parse::Parsed,
    kind: Kind,
    angle_style: AngleStyle,
    function_of_y: bool,
) -> Result<Id, Error> {
    let function = kind == Kind::Function;
    // `AlgebraProcessor.processNumber`: independent unless it reads an
    // object; a command such as `Mod(7, 3)` makes a helper object, so an
    // expression with one is dependent even on constants.
    let has_command = parsed.nodes.iter().any(|n| {
        matches!(n, Node::Bin(op, ..) if op.is_command())
            || matches!(
                n,
                Node::Cmd(..) | Node::If(IfMode::Algo, ..) | Node::Vector(_) | Node::Cas(..)
            )
            || matches!(n, Node::Financial(..))
            || is_list_node(n)
    });
    // a constant boolean (`b = 1 < 2`) or list stays an expression: there
    // is no free boolean or list object
    let constant = parsed.parents.is_empty()
        && !function
        && !has_command
        && !matches!(
            kind,
            Kind::Boolean | Kind::List | Kind::Text | Kind::FunctionNVar
        );
    let vector_label = kind == Kind::Vector;
    let mut entry = Entry::new(c, parsed, function);
    entry.function_of_y = function_of_y;
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
            Raw::List(_) | Raw::Num(_) | Raw::Vec(..) | Raw::Homogeneous(_) => None,
        };
        if let Some(def) = free {
            let id = c.add(name, def)?;
            if let (Some(d), Kind::Number | Kind::Angle, Raw::Num(v)) = (dec, kind, raw) {
                if let Some(d) = d.get(v) {
                    set_free_decimal(c, id, d.into_owned());
                }
            }
            return Ok(id);
        }
    }
    add_entry(c, name, entry, kind, angle_style, constant)
}

fn add_entry(
    c: &mut Construction,
    name: &str,
    entry: Entry,
    kind: Kind,
    angle_style: AngleStyle,
    independent: bool,
) -> Result<Id, Error> {
    let index = u32::try_from(c.exprs.entries.len()).map_err(|_| Error::UnknownObject)?;
    // AlgoIf and AlgoListElement copy existing points. Arithmetic point
    // expressions go through AlgoDependentPoint and reject infinity.
    let allows_infinite_point = independent
        || matches!(
            entry.nodes[entry.root as usize],
            Node::If(IfMode::Algo, ..) | Node::ListOp(ListOp::Element, ..)
        );
    c.exprs.entries.push(entry);
    c.exprs.pending = Some(index);
    let result = c.add(
        name,
        Def::Expr(ExprDef {
            kind,
            index,
            angle_style,
            allows_infinite_point,
        }),
    );
    c.exprs.pending = None;
    if result.is_err() {
        c.exprs.entries.pop();
    }
    result
}

#[cfg(test)]
mod tests;
