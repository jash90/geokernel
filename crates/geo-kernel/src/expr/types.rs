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

use super::{Bin, Elem, IfMode, ListOp, Node, Ty, Un, NONE};
use crate::algo::measure::MeasureDef;
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
            Node::Degree => (Ty::Num(1), 1, 1, 0),
            Node::Var => (Ty::Num(0), 1, 1, 1),
            Node::Obj(id) => (object_type(c, id)?, 1, 1, 0),
            Node::Cmd(def, _) => (result_type(def.kind(), is_angle_def(c, &def))?, 1, 1, 0),
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
                (
                    Ty::Num(0),
                    fd.saturating_add(depth[a]),
                    fc.saturating_add(fv.saturating_mul(cost[a])),
                    fv.saturating_mul(vars[a]),
                )
            }
            Node::Local(_) => (Ty::Num(0), 1, 1, 0),
            Node::Item(a) => {
                let a = a as usize;
                (tys[a], depth[a], cost[a], vars[a])
            }
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
                if n != NONE && !matches!(tys[n as usize], Ty::Num(_) | Ty::Bool) {
                    return Err("the index must be a number".into());
                }
                let ty = match op {
                    ListOp::Length => Ty::Num(0),
                    ListOp::Sum => match elem {
                        Elem::Num | Elem::Bool | Elem::Empty => Ty::Num(0),
                        _ => return Err("only the sum of numbers is supported".into()),
                    },
                    ListOp::Index | ListOp::Element => ty_of(elem)?,
                };
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

/// Whether a number object is an angle: GeoGebra gives angles one angle
/// dimension, which makes `floor`, `ceil` and `round` work in degrees.
fn is_angle_def(c: &Construction, def: &Def) -> bool {
    match *def {
        Def::Measure(MeasureDef::Angle(..)) | Def::Angle(_) => true,
        Def::Expr(d) => super::is_angle(c, d),
        _ => false,
    }
}

fn object_type(c: &Construction, id: Id) -> Result<Ty, String> {
    match (c.kind(id), c.def(id)) {
        (Kind::List, Def::Expr(d)) => Ok(super::entry(c, d).root_ty()),
        (kind, def) => result_type(kind, is_angle_def(c, &def)),
    }
}

/// The element type of a list of elements of type `t`.
fn elem_of(t: Ty) -> Elem {
    match t {
        Ty::Num(_) => Elem::Num,
        Ty::Vec => Elem::Vec,
        Ty::Bool => Elem::Bool,
        Ty::List(_) => Elem::List,
    }
}

fn join_elem(a: Elem, b: Elem) -> Elem {
    match (a, b) {
        (Elem::Empty, t) | (t, Elem::Empty) => t,
        (a, b) if a == b => a,
        _ => Elem::Any,
    }
}

/// The type of one element of a list (`GeoList.createTemplateElement` for
/// an empty one: a number).
fn ty_of(e: Elem) -> Result<Ty, String> {
    match e {
        Elem::Num | Elem::Empty => Ok(Ty::Num(0)),
        Elem::Vec => Ok(Ty::Vec),
        Elem::Bool => Ok(Ty::Bool),
        Elem::List => Ok(Ty::List(Elem::Any)),
        Elem::Any => Err("the elements of the list have different types".into()),
    }
}

fn result_type(kind: Kind, angle: bool) -> Result<Ty, String> {
    match kind {
        Kind::Number => Ok(Ty::Num(i8::from(angle))),
        Kind::Point | Kind::Vector => Ok(Ty::Vec),
        Kind::Boolean => Ok(Ty::Bool),
        other => Err(format!("a {other} cannot be used in an expression")),
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
    }
}

/// `MyDouble.add`, `mult`, `div`, `pow` for the angle dimension; vectors
/// add, scale and multiply to the scalar product.
fn binary_type(op: Bin, a: Ty, b: Ty) -> Result<Ty, String> {
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

fn scalar_binary_type(op: Bin, a: Ty, b: Ty) -> Result<Ty, String> {
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
