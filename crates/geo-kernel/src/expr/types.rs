// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Static types of an expression tree: numbers with their angle dimension,
//! points or vectors, and booleans; plus the size of the evaluation.
//!
//! GeoGebra finds a misplaced boolean only when it evaluates the line (a
//! `MyError` from `Operation`); the evaluation of a new object always runs,
//! so rejecting the same trees here gives the same lines errors.

use super::commands;
use super::{AlgebraOp, Bin, Elem, IfMode, Node, Ty, Un, NONE};
use crate::kernel::{Construction, Def, Id, Kind};

/// Types, depths, costs and variable counts of the nodes reachable from
/// `root`; children always come before their parents in the arena, and
/// nodes left behind by a rewrite are skipped.
#[allow(clippy::type_complexity)]
pub(crate) fn check(
    c: &Construction,
    nodes: &[Node],
    root: u32,
) -> Result<(Vec<Ty>, Vec<u32>, Vec<u64>, Vec<u64>), String> {
    let n = nodes.len();
    let mut reachable = vec![false; n];
    reachable[root as usize] = true;
    for i in (0..n).rev() {
        if reachable[i] {
            for child in super::children(&nodes[i]) {
                reachable[child as usize] = true;
            }
        }
    }
    let mut tys: Vec<Ty> = Vec::with_capacity(n);
    let mut depth: Vec<u32> = Vec::with_capacity(n);
    let mut cost: Vec<u64> = Vec::with_capacity(n);
    let mut vars: Vec<u64> = Vec::with_capacity(n);
    for (i, node) in nodes.iter().enumerate() {
        if !reachable[i] {
            tys.push(Ty::Num(0));
            depth.push(0);
            cost.push(0);
            vars.push(0);
            continue;
        }
        let (ty, d, k, v) = match *node {
            Node::Num(_) | Node::MinusOne => (Ty::Num(0), 1, 1, 0),
            Node::Bool(_) => (Ty::Bool, 1, 1, 0),
            Node::Text | Node::Relation { .. } => (Ty::Text, 1, 1, 0),
            Node::TextCmd(op, first, count) => {
                let end = (first + count) as usize;
                let first = first as usize;
                let ty = commands::text::result_type(c, op, nodes, &tys, first..end)?;
                let v = vars[first..end]
                    .iter()
                    .fold(0u64, |a, b| a.saturating_add(*b));
                let k = cost[first..end]
                    .iter()
                    .fold(1u64, |a, b| a.saturating_add(*b));
                (
                    ty,
                    1 + depth[first..end].iter().copied().max().unwrap_or(0),
                    k,
                    v,
                )
            }
            Node::Base { to, value, radix } => {
                let (v, r) = (value as usize, radix as usize);
                if !matches!(tys[r], Ty::Num(_))
                    || if to {
                        !matches!(tys[v], Ty::Num(_))
                    } else {
                        tys[v] != Ty::Text
                    }
                    || vars[v].saturating_add(vars[r]) != 0
                {
                    return Err("invalid base conversion arguments".into());
                }
                let ty = if to { Ty::Text } else { Ty::Num(0) };
                (
                    ty,
                    1 + depth[v].max(depth[r]),
                    cost[v].saturating_add(cost[r]).saturating_add(1),
                    0,
                )
            }
            Node::Side(..) => (Ty::Num(0), 1, 1, 0),
            Node::Apply2(_, a, b) => {
                let (a, b) = (a as usize, b as usize);
                if !matches!((tys[a], tys[b]), (Ty::Num(_), Ty::Num(_))) {
                    return Err("function coordinates must be numbers".into());
                }
                (
                    Ty::Num(0),
                    1 + depth[a].max(depth[b]),
                    1u64.saturating_add(cost[a]).saturating_add(cost[b]),
                    vars[a].saturating_add(vars[b]),
                )
            }
            Node::FunctionObject(_) => (Ty::Function, 1, 1, 0),
            Node::Degree => (Ty::Num(1), 1, 1, 0),
            Node::Var | Node::VarY => (Ty::Num(0), 1, 1, 1),
            Node::Obj(id) => (object_type(c, id)?, 1, 1, 0),
            Node::Cmd(def, _) => (result_type(def.kind())?, 1, 1, 0),
            Node::FnName(_) => return Err("a function name needs an argument".into()),
            Node::Tuple(a, b) | Node::Polar(a, b) => {
                let (a, b) = (a as usize, b as usize);
                if !matches!(tys[a], Ty::Num(_) | Ty::Bool)
                    || !matches!(tys[b], Ty::Num(_) | Ty::Bool)
                {
                    return Err("coordinates must be numbers".into());
                }
                (
                    Ty::Vec,
                    1 + depth[a].max(depth[b]),
                    1u64.saturating_add(cost[a]).saturating_add(cost[b]),
                    vars[a].saturating_add(vars[b]),
                )
            }
            Node::Un(op, a) => {
                let a = a as usize;
                (
                    unary_type(op, tys[a])?,
                    1 + depth[a],
                    cost[a].saturating_add(1),
                    vars[a],
                )
            }
            Node::Bin(op, a, b) => {
                let (a, b) = (a as usize, b as usize);
                if op.is_command() && vars[a].saturating_add(vars[b]) > 0 {
                    // GeoGebra rejects `Mod(x, 3)` in a function
                    return Err("a command cannot take `x`".into());
                }
                (
                    binary_type(op, tys[a], tys[b])?,
                    1 + depth[a].max(depth[b]),
                    1u64.saturating_add(cost[a]).saturating_add(cost[b]),
                    vars[a].saturating_add(vars[b]),
                )
            }
            Node::Algebra(op, a, b) => {
                let (a, b) = (a as usize, b as usize);
                if !matches!(tys[a], Ty::Num(_))
                    || b != NONE as usize && !matches!(tys[b], Ty::Num(_))
                {
                    return Err("the command needs numeric arguments".into());
                }
                let (bd, bc, bv) = if b == NONE as usize {
                    (0, 0, 0)
                } else {
                    (depth[b], cost[b], vars[b])
                };
                if vars[a].saturating_add(bv) != 0 {
                    return Err("a command cannot take `x`".into());
                }
                let ty = if op.makes_list() {
                    Ty::List(Elem::Num)
                } else if matches!(op, AlgebraOp::IsPrime | AlgebraOp::IsInteger) {
                    Ty::Bool
                } else {
                    Ty::Num(0)
                };
                (
                    ty,
                    1 + depth[a].max(bd),
                    cost[a].saturating_add(bc).saturating_add(1),
                    0,
                )
            }
            Node::Extremum(_, f, a, b) => {
                let [f, a, b] = [f, a, b].map(|i| i as usize);
                if [f, a, b].iter().any(|&i| !matches!(tys[i], Ty::Num(_))) {
                    return Err("an extremum needs a numeric function and two numbers".into());
                }
                if vars[a] != 0 || vars[b] != 0 {
                    return Err("an extremum needs fixed bounds".into());
                }
                (
                    Ty::Vec,
                    1 + depth[f].max(depth[a]).max(depth[b]),
                    1u64.saturating_add(cost[f])
                        .saturating_add(cost[a])
                        .saturating_add(cost[b]),
                    0,
                )
            }
            Node::PolynomialPart(_, a, b) => {
                let (a, b) = (a as usize, b as usize);
                if !matches!((tys[a], tys[b]), (Ty::Num(_), Ty::Num(_))) {
                    return Err("polynomial division needs numeric functions".into());
                }
                (
                    Ty::Num(0),
                    1 + depth[a].max(depth[b]),
                    1u64.saturating_add(cost[a]).saturating_add(cost[b]),
                    1,
                )
            }
            Node::If(mode, cond, then, rest) => {
                let (cond, then) = (cond as usize, then as usize);
                if tys[cond] != Ty::Bool {
                    // `CmdIf`: `argErr` for a condition that is no boolean
                    return Err("the condition of `If` must be a boolean".into());
                }
                let (ty, d, k, v) = if rest == NONE {
                    (tys[then], 0, 0, 0)
                } else {
                    let r = rest as usize;
                    (join(tys[then], tys[r])?, depth[r], cost[r], vars[r])
                };
                if mode != IfMode::Algo && !matches!(ty, Ty::Num(_)) {
                    // `CmdIf.checkAdd`: the branches must be functions
                    return Err("the branches of a function's `If` must be numbers".into());
                }
                (
                    ty,
                    1 + depth[cond].max(depth[then]).max(d),
                    1u64.saturating_add(cost[cond])
                        .saturating_add(cost[then])
                        .saturating_add(k),
                    vars[cond].saturating_add(vars[then]).saturating_add(v),
                )
            }
            Node::Apply(f, a) => {
                let a = a as usize;
                if !matches!(tys[a], Ty::Num(_) | Ty::Bool) {
                    return Err("a function takes a number".into());
                }
                let (fd, fc, fv) = super::function_metrics(c, f);
                let ty = if super::function_signature(c, f).is_some_and(|f| f.boolean) {
                    Ty::Bool
                } else {
                    Ty::Num(0)
                };
                (
                    ty,
                    fd.saturating_add(depth[a]),
                    1u64.saturating_add(fc)
                        .saturating_add(fv.max(1).saturating_mul(cost[a])),
                    fv.saturating_mul(vars[a]),
                )
            }
            Node::Inverse(a) => {
                let a = a as usize;
                if !matches!(tys[a], Ty::Num(_)) {
                    return Err("`Invert` needs a function or matrix".into());
                }
                // Inverting an inverse visits its source to check for an
                // inverse and again to evaluate the original function.
                (
                    Ty::Num(0),
                    1 + depth[a],
                    cost[a].saturating_mul(2).saturating_add(1),
                    1,
                )
            }
            Node::Dilate(src, factor, center) => {
                let (s, f, p) = (src as usize, factor as usize, center as usize);
                if !matches!(tys[f], Ty::Num(_))
                    || center != NONE
                        && (tys[p] != Ty::Vec || vec_kind(c, nodes, &tys, center) != Kind::Point)
                {
                    return Err("`Dilate` needs a numeric factor and an optional point".into());
                }
                let ty = match tys[s] {
                    ty @ (Ty::List(_) | Ty::Bool) => ty,
                    Ty::Num(_) => Ty::Num(0),
                    _ => return Err("expected a list or a function".into()),
                };
                let (pd, pc, pv) = if center == NONE {
                    (0, 0, 0)
                } else {
                    (depth[p], cost[p], vars[p])
                };
                (
                    ty,
                    1 + depth[s].max(depth[f]).max(pd),
                    cost[s]
                        .saturating_add(cost[f])
                        .saturating_add(pc)
                        .saturating_add(1),
                    vars[s].saturating_add(vars[f]).saturating_add(pv),
                )
            }
            Node::Local(_, elem) => (ty_of(elem)?, 1, 1, 0),
            Node::ObjectDefined(_) => (Ty::Bool, 1, 1, 0),
            Node::Defined(a) => {
                let a = a as usize;
                (
                    Ty::Bool,
                    1 + depth[a],
                    1u64.saturating_add(cost[a]),
                    vars[a],
                )
            }
            Node::Filter {
                keep,
                list,
                condition,
                ..
            } => {
                let (l, c) = (list as usize, condition as usize);
                let Ty::List(elem) = tys[l] else {
                    return Err("the filter needs a list".into());
                };
                if tys[c] != Ty::Bool {
                    return Err("the filter needs a boolean condition".into());
                }
                let ty = if keep { Ty::List(elem) } else { Ty::Num(0) };
                (
                    ty,
                    1 + depth[l].max(depth[c]),
                    cost[l].saturating_add(cost[c]).saturating_add(1),
                    vars[l].saturating_add(vars[c]),
                )
            }
            Node::Vector(a) => {
                let a = a as usize;
                (Ty::Vec, 1 + depth[a], cost[a].saturating_add(1), vars[a])
            }
            Node::Item(a) => {
                let a = a as usize;
                (tys[a], depth[a], cost[a], vars[a])
            }
            Node::ItemObject(_) => (Ty::Object, 1, 1, 0),
            Node::List(first, len) => {
                let mut elem = Elem::Empty;
                let (mut d, mut k, mut v) = (0u32, 1u64, 0u64);
                for item in first..first.saturating_add(len) {
                    let item = item as usize;
                    elem = join_elem(elem, elem_of(tys[item]));
                    d = d.max(depth[item]);
                    k = k.saturating_add(cost[item]);
                    v = v.saturating_add(vars[item]);
                }
                (Ty::List(elem), 1 + d, k, v)
            }
            Node::ListOp(op, l, n) => {
                let Ty::List(elem) = tys[l as usize] else {
                    return Err("the command needs a list".into());
                };
                let object = commands::takes_object(op);
                if n != NONE && !object && !matches!(tys[n as usize], Ty::Num(_) | Ty::Bool) {
                    return Err("the index must be a number".into());
                }
                // a list of numbers and booleans has mixed elements
                let numbers = matches!(elem, Elem::Num | Elem::Bool | Elem::Empty | Elem::Any);
                let ty = commands::list_type(op, elem, numbers, nodes, &tys, l, n)?;
                let (l, n) = (l as usize, n as usize);
                let (nd, nc, nv) = if n == NONE as usize {
                    (0, 0, 0)
                } else {
                    (depth[n], cost[n], vars[n])
                };
                (
                    ty,
                    1 + depth[l].max(nd),
                    1u64.saturating_add(cost[l]).saturating_add(nc),
                    vars[l].saturating_add(nv),
                )
            }
            Node::Range(..) | Node::Seq(..) => {
                let (args, elem): (&[u32], Elem) = match *node {
                    Node::Range(a, b, c) => (&[a, b, c], Elem::Num),
                    Node::Seq(e, a, b, c, _) => (&[a, b, c, e], elem_of(tys[e as usize])),
                    _ => unreachable!("matched above"),
                };
                let (mut d, mut k, mut v) = (0u32, 1u64, 0u64);
                for (j, &a) in args.iter().enumerate() {
                    if a == NONE {
                        continue;
                    }
                    let a = a as usize;
                    if j < 3 && !matches!(tys[a], Ty::Num(_) | Ty::Bool) {
                        return Err("the bounds of `Sequence` must be numbers".into());
                    }
                    d = d.max(depth[a]);
                    k = k.saturating_add(cost[a]);
                    v = v.saturating_add(vars[a]);
                }
                (Ty::List(elem), 1 + d, k, v)
            }
        };
        tys.push(ty);
        depth.push(d);
        cost.push(k);
        vars.push(v);
    }
    Ok((tys, depth, cost, vars))
}

fn object_type(c: &Construction, id: Id) -> Result<Ty, String> {
    match (c.kind(id), c.def(id)) {
        (Kind::List, Def::Expr(d)) => Ok(super::entry(c, d).root_ty()),
        (kind, _) => result_type(kind),
    }
}

/// A coordinate expression keeps the kind of an object or command. Bare
/// coordinates are points, as are operations that have a point operand.
pub(super) fn vec_kind(c: &Construction, nodes: &[Node], tys: &[Ty], i: u32) -> Kind {
    let kind = |k| if k == Kind::Vector { k } else { Kind::Point };
    match nodes[i as usize] {
        Node::Obj(id) => kind(c.kind(id)),
        Node::Cmd(def, _) => kind(def.kind()),
        Node::Vector(_) => Kind::Vector,
        Node::Item(a) | Node::Un(_, a) | Node::If(_, _, a, _) => vec_kind(c, nodes, tys, a),
        Node::Bin(_, a, b) => {
            let kinds =
                [a, b].map(|j| (tys[j as usize] == Ty::Vec).then(|| vec_kind(c, nodes, tys, j)));
            if kinds.contains(&Some(Kind::Point)) || !kinds.contains(&Some(Kind::Vector)) {
                Kind::Point
            } else {
                Kind::Vector
            }
        }
        _ => Kind::Point,
    }
}

/// The element type of a list of elements of type `t`.
pub(super) fn elem_of(t: Ty) -> Elem {
    match t {
        Ty::Num(_) => Elem::Num,
        Ty::Vec => Elem::Vec,
        Ty::Bool => Elem::Bool,
        Ty::Function => Elem::Function,
        Ty::Text => Elem::Text,
        Ty::List(_) => Elem::List,
        Ty::Object => Elem::Object,
    }
}

pub(super) fn join_elem(a: Elem, b: Elem) -> Elem {
    match (a, b) {
        (Elem::Empty, t) | (t, Elem::Empty) => t,
        (a, b) if a == b => a,
        _ => Elem::Any,
    }
}

/// The type of one element of a list (`GeoList.createTemplateElement` for
/// an empty one: a number).
pub(super) fn ty_of(e: Elem) -> Result<Ty, String> {
    match e {
        Elem::Num | Elem::Empty => Ok(Ty::Num(0)),
        Elem::Vec => Ok(Ty::Vec),
        Elem::Bool => Ok(Ty::Bool),
        Elem::Function => Ok(Ty::Function),
        Elem::Text => Ok(Ty::Text),
        Elem::List => Ok(Ty::List(Elem::Any)),
        Elem::Object => Err("geometric list elements cannot be used as operands".into()),
        Elem::Any => Err("the elements of the list have different types".into()),
    }
}

/// The number that a command reading numbers gives (`AlgoStats1D`,
/// `AlgoListMinMax`); an error unless the elements may all be numbers.
pub(super) fn of_numbers(numbers: bool) -> Result<Ty, String> {
    if !numbers {
        return Err("the command needs a list of numbers".into());
    }
    Ok(Ty::Num(0))
}

fn result_type(kind: Kind) -> Result<Ty, String> {
    match kind {
        Kind::Number => Ok(Ty::Num(0)),
        Kind::Angle => Ok(Ty::Num(1)),
        Kind::Point | Kind::Vector => Ok(Ty::Vec),
        Kind::Boolean => Ok(Ty::Bool),
        Kind::Text => Ok(Ty::Text),
        _ => Ok(Ty::Object),
    }
}

/// `MyDouble`: functions drop the angle dimension except `abs`, `floor`,
/// `ceil` and `round`; complex arguments are not supported.
fn unary_type(op: Un, a: Ty) -> Result<Ty, String> {
    if let Ty::List(e) = a {
        // `MyList.apply`: element by element
        return Ok(Ty::List(elem_of(scalar_unary_type(op, ty_of(e)?)?)));
    }
    scalar_unary_type(op, a)
}

fn scalar_unary_type(op: Un, a: Ty) -> Result<Ty, String> {
    if op == Un::Not {
        // `Operation.NOT`: `illegalBoolean` for anything else
        return match a {
            Ty::Bool => Ok(Ty::Bool),
            _ => Err("`\u{ac}` needs a boolean".into()),
        };
    }
    match (op, numeric(a)) {
        (Un::X | Un::Y, Ty::Vec) => Ok(Ty::Num(0)),
        (Un::X | Un::Y, Ty::Num(_)) => Err("`x()` and `y()` need a point or a vector".into()),
        (Un::Abs, Ty::Vec) => Ok(Ty::Num(0)),
        (_, Ty::Vec) => Err("complex numbers are not supported".into()),
        (Un::Abs | Un::Floor | Un::Ceil | Un::Round, Ty::Num(d)) => Ok(Ty::Num(d)),
        // `MyDouble.asin(true)`: `makeAngle`
        (Un::AsinD | Un::AcosD | Un::AtanD, Ty::Num(_)) => Ok(Ty::Num(1)),
        (_, Ty::Num(_) | Ty::Bool) => Ok(Ty::Num(0)),
        (_, Ty::List(_)) => Err("a list of lists cannot be used here".into()),
        (_, Ty::Object) => Err("geometric list elements cannot be used as operands".into()),
        (_, Ty::Function | Ty::Text) => Err("this operand needs a number".into()),
    }
}

/// `MyDouble.add`, `mult`, `div`, `pow` for the angle dimension; vectors
/// add, scale and multiply to the scalar product.
fn binary_type(op: Bin, a: Ty, b: Ty) -> Result<Ty, String> {
    if a == Ty::Text || b == Ty::Text {
        return if op == Bin::Add {
            Ok(Ty::Text)
        } else {
            Err("text operands require concatenation".into())
        };
    }
    if a == Ty::Object || b == Ty::Object {
        return Err("geometric objects cannot be used as arithmetic operands".into());
    }
    let list = |t: Ty| match t {
        Ty::List(e) => ty_of(e).map(Some),
        _ => Ok(None),
    };
    // `handleMinus`: a list minus a point or vector, or the other way
    // round, reads the list as one vector (`GeoVec2D.sub`), whatever its
    // elements are
    let is_list = |t: Ty| matches!(t, Ty::List(_));
    if op == Bin::Sub && is_list(a) != is_list(b) && (a == Ty::Vec || b == Ty::Vec) {
        return Ok(Ty::Vec);
    }
    match (list(a)?, list(b)?) {
        (None, None) => scalar_binary_type(op, a, b),
        _ if op.is_comparison() || op.is_logic() => Err("comparing lists is not supported".into()),
        // `MyList.apply`: element by element, or every element with the
        // other operand
        (la, lb) => {
            let t = scalar_binary_type(op, la.unwrap_or(a), lb.unwrap_or(b))?;
            if matches!(t, Ty::List(_)) {
                return Err("arithmetic on lists of lists is not supported".into());
            }
            Ok(Ty::List(elem_of(t)))
        }
    }
}

pub(super) fn scalar_binary_type(op: Bin, a: Ty, b: Ty) -> Result<Ty, String> {
    use Ty::{Num, Vec};
    if matches!(a, Ty::List(_)) || matches!(b, Ty::List(_)) {
        return Err("arithmetic on lists of lists is not supported".into());
    }
    if op.is_logic() {
        // `Operation.AND` and the others: `IllegalBoolean` unless both are
        return match (a, b) {
            (Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
            _ => Err("a logical operator needs two booleans".into()),
        };
    }
    if op.is_comparison() {
        // `==` and `!=` compare anything (a number and a point are not
        // equal); `<` and the others only numbers (`illegalComparison`)
        let ordered = !matches!(op, Bin::Equal | Bin::NotEqual);
        return if ordered && (a == Vec || b == Vec) {
            Err("only numbers can be ordered".into())
        } else {
            Ok(Ty::Bool)
        };
    }
    let complex = || Err("complex numbers are not supported".to_string());
    match (op, numeric(a), numeric(b)) {
        (Bin::Add | Bin::Sub, Num(x), Num(y)) => Ok(Num(if x == y { x } else { 0 })),
        (Bin::Add | Bin::Sub, _, _) => Ok(Vec),
        (Bin::Mul, Num(x), Num(y)) => Ok(Num(x.saturating_add(y))),
        (Bin::Mul, Vec, Vec) => Ok(Num(0)),
        (Bin::Mul, _, _) => Ok(Vec),
        (Bin::Div, Num(x), Num(y)) => Ok(Num(x.saturating_sub(y))),
        (Bin::Div, Vec, Num(_)) => Ok(Vec),
        (Bin::Pow, Num(x), Num(y)) => Ok(Num(if y > 0 { 0 } else { x })),
        (Bin::Pow, Vec, Num(_)) => Ok(Num(0)),
        (Bin::Round2, Num(x), Num(_)) => Ok(Num(x)),
        (Bin::Atan2 | Bin::LogB, Num(_), Num(_)) => Ok(Num(0)),
        (op, Num(_), Num(_)) if op.is_command() => Ok(Num(0)),
        (op, _, _) if op.is_command() => Err("the command takes two numbers".into()),
        _ => complex(),
    }
}

/// The type of `AlgoIf`'s result: `GeoList.getGenericElement` keeps the
/// first branch's type unless a later one can take it, which a point does
/// for a number.
fn join(a: Ty, b: Ty) -> Result<Ty, String> {
    match (a, b) {
        (Ty::Num(x), Ty::Num(_)) => Ok(Ty::Num(x)),
        (Ty::Vec | Ty::Num(_), Ty::Vec) | (Ty::Vec, Ty::Num(_)) => Ok(Ty::Vec),
        (Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
        (Ty::List(x), Ty::List(y)) => Ok(Ty::List(join_elem(x, y))),
        _ => Err("the branches of `If` do not fit together".into()),
    }
}

/// A boolean where a number is expected is the number 1 or 0.
fn numeric(t: Ty) -> Ty {
    match t {
        Ty::Bool => Ty::Num(0),
        t => t,
    }
}
