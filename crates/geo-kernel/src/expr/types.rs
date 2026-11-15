// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyDouble.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Static types of an expression tree: numbers with their angle dimension,
//! points or vectors, and booleans; plus the size of the evaluation.
//!
//! GeoGebra finds a misplaced boolean only when it evaluates the line (a
//! `MyError` from `Operation`); the evaluation of a new object always runs,
//! so rejecting the same trees here gives the same lines errors.

use super::{Bin, IfMode, Node, Ty, Un, NONE};
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
                if tys[a] == Ty::Vec || tys[b] == Ty::Vec {
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
                if tys[a] == Ty::Vec {
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
    result_type(c.kind(id), is_angle_def(c, &c.def(id)))
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
    }
}

/// `MyDouble.add`, `mult`, `div`, `pow` for the angle dimension; vectors
/// add, scale and multiply to the scalar product.
fn binary_type(op: Bin, a: Ty, b: Ty) -> Result<Ty, String> {
    use Ty::{Num, Vec};
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
