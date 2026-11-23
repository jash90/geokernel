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
use super::{AlgebraOp, Bin, CallOp, Elem, IfMode, ListOp, Node, Ty, Un, NONE};
use crate::kernel::{Construction, Def, Id, Kind};

/// Types, depths, costs and variable counts of the nodes reachable from
/// `root`; children always come before their parents in the arena, and
/// nodes left behind by a rewrite are skipped.
#[allow(clippy::type_complexity)]
/// The types, depths, costs and variable counts of the nodes typed so far.
struct State {
    tys: Vec<Ty>,
    depth: Vec<u32>,
    cost: Vec<u64>,
    vars: Vec<u64>,
    /// the type of the variable of every `Zip` whose expression is being
    /// typed (`GeoList.createTemplateElement`), newest last; scoped to it
    zip_locals: Vec<(u32, Ty)>,
}

impl State {
    /// The type of the variable of the `Zip` of `depth`: the innermost one
    /// that binds it, the element its parse-time type names otherwise.
    fn local(&self, depth: u32, elem: Elem) -> Result<Ty, String> {
        match self
            .zip_locals
            .iter()
            .rev()
            .find(|&&(d, _)| d == depth)
            .map(|&(_, t)| t)
        {
            Some(t) => Ok(t),
            None => ty_of(elem),
        }
    }
}

/// What `check` returns: the type, depth, cost and variable count of every
/// node, in the order of the arena.
pub(crate) type Checked = (Vec<Ty>, Vec<u32>, Vec<u64>, Vec<u64>);

pub(crate) fn check(c: &Construction, nodes: &[Node], root: u32) -> Result<Checked, String> {
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
    let mut state = State {
        tys: Vec::with_capacity(n),
        depth: Vec::with_capacity(n),
        cost: Vec::with_capacity(n),
        vars: Vec::with_capacity(n),
        zip_locals: Vec::new(),
    };
    // the expressions of `Zip` calls are typed by their own arm, with the
    // variables bound to the elements of their lists
    let mut in_zip = vec![false; n];
    for node in nodes {
        if let Node::Call(CallOp::Zip, first, _) = *node {
            if let Node::Item(e) = nodes[first as usize] {
                for j in subtree_min(nodes, e)..=e {
                    in_zip[j as usize] = true;
                }
            }
        }
    }
    for i in 0..n {
        state.tys.push(Ty::Num(0));
        state.depth.push(0);
        state.cost.push(0);
        state.vars.push(0);
        if !reachable[i] || in_zip[i] {
            continue;
        }
        type_node(c, nodes, &mut state, i as u32)?;
    }
    Ok((state.tys, state.depth, state.cost, state.vars))
}

/// The first node of the subtree of `root` (its children come before it).
fn subtree_min(nodes: &[Node], root: u32) -> u32 {
    let mut min = root;
    for child in super::children(&nodes[root as usize]) {
        min = min.min(subtree_min(nodes, child));
    }
    min
}

/// Types node `i` from the types of its children (which come before it in
/// the arena), assigns the result to `state` and returns it.
fn type_node(
    c: &Construction,
    nodes: &[Node],
    state: &mut State,
    i: u32,
) -> Result<(Ty, u32, u64, u64), String> {
    let i = i as usize;
    let (ty, d, k, v) = match nodes[i] {
        Node::Num(_) | Node::MinusOne | Node::CasVariable(_) => (Ty::Num(0), 1, 1, 0),
        Node::Bool(_) => (Ty::Bool, 1, 1, 0),
        Node::Text | Node::Relation { .. } => (Ty::Text, 1, 1, 0),
        Node::TextCmd(op, first, count) => {
            let end = (first + count) as usize;
            let first = first as usize;
            let ty = commands::text::result_type(c, op, nodes, &state.tys, first..end)?;
            let v = state.vars[first..end]
                .iter()
                .fold(0u64, |a, b| a.saturating_add(*b));
            let k = state.cost[first..end]
                .iter()
                .fold(1u64, |a, b| a.saturating_add(*b));
            (
                ty,
                1 + state.depth[first..end].iter().copied().max().unwrap_or(0),
                k,
                v,
            )
        }
        Node::Base { to, value, radix } => {
            let (v, r) = (value as usize, radix as usize);
            if !matches!(state.tys[r], Ty::Num(_))
                || if to {
                    !matches!(state.tys[v], Ty::Num(_))
                } else {
                    state.tys[v] != Ty::Text
                }
                || state.vars[v].saturating_add(state.vars[r]) != 0
            {
                return Err("invalid base conversion arguments".into());
            }
            let ty = if to { Ty::Text } else { Ty::Num(0) };
            (
                ty,
                1 + state.depth[v].max(state.depth[r]),
                1u64.saturating_add(state.cost[v])
                    .saturating_add(state.cost[r]),
                0,
            )
        }
        Node::Side(..) => (Ty::Num(0), 1, 1, 0),
        Node::Financial(_, args) => {
            let args = args
                .iter()
                .copied()
                .take_while(|&a| a != super::NONE)
                .map(|a| a as usize)
                .collect::<Vec<_>>();
            if args.iter().any(|&a| !matches!(state.tys[a], Ty::Num(_))) {
                return Err("a financial command needs numbers".into());
            }
            let depth = args.iter().map(|&a| state.depth[a]).max().unwrap_or(0);
            let cost = args
                .iter()
                .map(|&a| state.cost[a])
                .fold(2u64, |sum, c| sum.saturating_add(c))
                .saturating_mul(3);
            let vars = args.iter().map(|&a| state.vars[a]).sum();
            (Ty::Num(0), 1 + depth, cost, vars)
        }
        Node::Apply2(f, a, b) => {
            let (a, b) = (a as usize, b as usize);
            if !matches!((state.tys[a], state.tys[b]), (Ty::Num(_), Ty::Num(_))) {
                return Err("function coordinates must be numbers".into());
            }
            (
                if super::inequality::is_inequality(c, f) {
                    Ty::Bool
                } else {
                    Ty::Num(0)
                },
                1 + state.depth[a].max(state.depth[b]),
                1u64.saturating_add(state.cost[a])
                    .saturating_add(state.cost[b]),
                state.vars[a].saturating_add(state.vars[b]),
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
            let coords = [state.tys[a], state.tys[b]];
            let scalar = |ty| matches!(ty, Ty::Num(_) | Ty::Bool);
            let numeric_list = |ty| {
                matches!(
                    ty,
                    Ty::List(Elem::Num | Elem::Bool | Elem::Empty | Elem::Any)
                )
            };
            let ty = if coords.into_iter().all(scalar) {
                Ty::Vec
            } else if matches!(nodes[i], Node::Tuple(..))
                && coords.into_iter().all(|ty| scalar(ty) || numeric_list(ty))
            {
                Ty::List(Elem::Vec)
            } else {
                return Err("coordinates must be numbers".into());
            };
            (
                ty,
                1 + state.depth[a].max(state.depth[b]),
                1u64.saturating_add(state.cost[a])
                    .saturating_add(state.cost[b]),
                state.vars[a].saturating_add(state.vars[b]),
            )
        }
        Node::Un(op, a) => {
            let a = a as usize;
            // A segment is a number, its length (see `Node::Bin` below).
            let ta = match (nodes[a], state.tys[a]) {
                (Node::Obj(id), Ty::Object) if c.kind(id) == Kind::Segment => Ty::Num(0),
                (_, t) => t,
            };
            (
                unary_type(op, ta)?,
                1 + state.depth[a],
                state.cost[a].saturating_add(1),
                state.vars[a],
            )
        }
        Node::Bin(op, a, b) => {
            let (a, b) = (a as usize, b as usize);
            if op.is_command() && state.vars[a].saturating_add(state.vars[b]) > 0 {
                // GeoGebra rejects `Mod(x, 3)` in a function
                return Err("a command cannot take `x`".into());
            }
            // A segment is a number, its length (`GeoSegment` is a
            // `NumberValue`); `==` and `!=` also compare geometric objects
            // (`ExpressionNode.isEqual`).
            let operand = |i: usize| match (nodes[i], state.tys[i]) {
                (Node::Obj(id), Ty::Object) if c.kind(id) == Kind::Segment => Ty::Num(0),
                (_, t) => t,
            };
            let (ta, tb) = (operand(a), operand(b));
            let simple = |t: Ty| matches!(t, Ty::Num(_) | Ty::Vec | Ty::Bool | Ty::Object);
            // Polygons, polylines, arcs and sectors are number values in the
            // reference (area or length), which `==` compares before their
            // geometry; the kernel has no such number in an expression, so
            // those comparisons stay refused.
            let numbered = |i: usize| {
                matches!(nodes[i], Node::Obj(id) if matches!(
                    c.kind(id),
                    Kind::Polygon | Kind::PolyLine | Kind::Arc | Kind::Sector
                ))
            };
            let ty = if matches!(op, Bin::Equal | Bin::NotEqual)
                && (ta == Ty::Object || tb == Ty::Object)
                && simple(ta)
                && simple(tb)
                && !numbered(a)
                && !numbered(b)
            {
                Ty::Bool
            } else {
                binary_type(op, ta, tb)?
            };
            (
                ty,
                1 + state.depth[a].max(state.depth[b]),
                1u64.saturating_add(state.cost[a])
                    .saturating_add(state.cost[b]),
                state.vars[a].saturating_add(state.vars[b]),
            )
        }
        Node::Algebra(op, a, b) => {
            let (a, b) = (a as usize, b as usize);
            if !matches!(state.tys[a], Ty::Num(_))
                || b != NONE as usize && !matches!(state.tys[b], Ty::Num(_))
            {
                return Err("the command needs numeric arguments".into());
            }
            let (bd, bc, bv) = if b == NONE as usize {
                (0, 0, 0)
            } else {
                (state.depth[b], state.cost[b], state.vars[b])
            };
            if state.vars[a].saturating_add(bv) != 0 {
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
                1 + state.depth[a].max(bd),
                state.cost[a].saturating_add(bc).saturating_add(1),
                0,
            )
        }
        Node::Extremum(_, f, a, b) => {
            let [f, a, b] = [f, a, b].map(|i| i as usize);
            if [f, a, b]
                .iter()
                .any(|&i| !matches!(state.tys[i], Ty::Num(_)))
            {
                return Err("an extremum needs a numeric function and two numbers".into());
            }
            if state.vars[a] != 0 || state.vars[b] != 0 {
                return Err("an extremum needs fixed bounds".into());
            }
            (
                Ty::Vec,
                1 + state.depth[f].max(state.depth[a]).max(state.depth[b]),
                1u64.saturating_add(state.cost[f])
                    .saturating_add(state.cost[a])
                    .saturating_add(state.cost[b]),
                0,
            )
        }
        Node::SystemFunction(arity, a) => {
            let a = a as usize;
            if !matches!(state.tys[a], Ty::Num(_)) && (arity != 2 || state.tys[a] != Ty::Bool) {
                return Err("a system function must give a number".into());
            }
            (
                state.tys[a],
                state.depth[a] + 1,
                state.cost[a].saturating_add(1),
                0,
            )
        }
        Node::FunctionAsymptotes(a) => {
            let a = a as usize;
            if !(matches!(state.tys[a], Ty::Num(_))
                || matches!(nodes[a], Node::Obj(id) if c.kind(id) == Kind::ImplicitCurve))
            {
                return Err("Asymptote needs a real function or implicit curve".into());
            }
            (
                Ty::List(Elem::Object),
                state.depth[a] + 1,
                state.cost[a].saturating_add(1),
                0,
            )
        }
        Node::VertexForm(a) => {
            let a = a as usize;
            let functionable = match nodes[a] {
                Node::Obj(id) => {
                    c.kind(id).is_number()
                        || matches!(
                            c.kind(id),
                            Kind::Line
                                | Kind::Ray
                                | Kind::Segment
                                | Kind::Conic
                                | Kind::Circle
                                | Kind::Arc
                                | Kind::Sector
                        )
                }
                Node::Apply(_, x) if matches!(nodes[x as usize], Node::Var) => true,
                _ => {
                    matches!(state.tys[a], Ty::Num(_))
                        || state.tys[a] == Ty::Bool && state.vars[a] > 0
                }
            };
            if !functionable {
                return Err("IsVertexForm needs a function, number, line or conic".into());
            }
            (
                Ty::Bool,
                state.depth[a] + 1,
                state.cost[a].saturating_add(1),
                0,
            )
        }
        Node::RootList(a) => {
            let a = a as usize;
            if !matches!(state.tys[a], Ty::List(_)) || state.vars[a] != 0 {
                return Err("RootList needs a list of values".into());
            }
            (
                Ty::List(Elem::Vec),
                state.depth[a] + 1,
                state.cost[a].saturating_add(1),
                0,
            )
        }
        Node::Iteration {
            mode,
            collect,
            formula,
            initial,
            count,
        } => {
            let [f, a, n] = [formula, initial, count].map(|i| i as usize);
            let ty = commands::function_analysis::iteration_type(
                mode,
                collect,
                state.tys[f],
                state.tys[a],
                state.tys[n],
            )?;
            if state.vars[a] != 0 || state.vars[n] != 0 {
                return Err("Iteration needs fixed initial values and count".into());
            }
            (
                ty,
                1 + state.depth[f].max(state.depth[a]).max(state.depth[n]),
                1u64.saturating_add(state.cost[f])
                    .saturating_add(state.cost[a])
                    .saturating_add(state.cost[n]),
                0,
            )
        }
        Node::PolynomialPart(_, a, b) => {
            let (a, b) = (a as usize, b as usize);
            if !matches!((state.tys[a], state.tys[b]), (Ty::Num(_), Ty::Num(_))) {
                return Err("polynomial division needs numeric functions".into());
            }
            (
                Ty::Num(0),
                1 + state.depth[a].max(state.depth[b]),
                1u64.saturating_add(state.cost[a])
                    .saturating_add(state.cost[b]),
                1,
            )
        }
        Node::If(mode, cond, then, rest) => {
            let (cond, then) = (cond as usize, then as usize);
            if state.tys[cond] != Ty::Bool {
                // `CmdIf`: `argErr` for a condition that is no boolean
                return Err("the condition of `If` must be a boolean".into());
            }
            let (ty, d, k, v) = if rest == NONE {
                (state.tys[then], 0, 0, 0)
            } else {
                let r = rest as usize;
                let ty = match mode {
                    IfMode::Algo => conditional_type(nodes, &state.tys, then, r),
                    IfMode::AlgoNext => state.tys[then],
                    _ => join(state.tys[then], state.tys[r])?,
                };
                (ty, state.depth[r], state.cost[r], state.vars[r])
            };
            if matches!(mode, IfMode::Else | IfMode::List) && !matches!(ty, Ty::Num(_)) {
                // `CmdIf.checkAdd`: the branches must be functions
                return Err("the branches of a function's `If` must be numbers".into());
            }
            (
                ty,
                1 + state.depth[cond].max(state.depth[then]).max(d),
                1u64.saturating_add(state.cost[cond])
                    .saturating_add(state.cost[then])
                    .saturating_add(k),
                state.vars[cond]
                    .saturating_add(state.vars[then])
                    .saturating_add(v),
            )
        }
        Node::Cas(op, a, b) => {
            let a = a as usize;
            let b = (b != NONE).then_some(b as usize);
            let ty = op.result_type(state.tys[a], b.map(|i| state.tys[i]))?;
            let vars = if op.is_function() {
                state.vars[a]
                    .saturating_add(b.map_or(0, |i| state.vars[i]))
                    .max(u64::from(
                        state.tys[a] == Ty::Function
                            || b.is_some_and(|i| state.tys[i] == Ty::Function),
                    ))
            } else if matches!(
                op,
                super::cas::Op::NextPrime | super::cas::Op::PreviousPrime
            ) {
                state.vars[a]
            } else {
                0
            };
            (
                ty,
                1 + state.depth[a].max(b.map_or(0, |i| state.depth[i])),
                1u64.saturating_add(state.cost[a])
                    .saturating_add(b.map_or(0, |i| state.cost[i])),
                vars,
            )
        }
        Node::Apply(f, a) => {
            let a = a as usize;
            if !matches!(state.tys[a], Ty::Num(_) | Ty::Bool) {
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
                fd.saturating_add(state.depth[a]),
                1u64.saturating_add(fc)
                    .saturating_add(fv.max(1).saturating_mul(state.cost[a])),
                fv.saturating_mul(state.vars[a]),
            )
        }
        Node::Inverse(a) => {
            let a = a as usize;
            if !matches!(state.tys[a], Ty::Num(_)) {
                return Err("`Invert` needs a function or matrix".into());
            }
            // Inverting an inverse visits its source to check for an
            // inverse and again to evaluate the original function.
            (
                Ty::Num(0),
                1 + state.depth[a],
                state.cost[a].saturating_mul(2).saturating_add(1),
                1,
            )
        }
        Node::Dilate(src, factor, center) => {
            let (s, f, p) = (src as usize, factor as usize, center as usize);
            if !matches!(state.tys[f], Ty::Num(_))
                || center != NONE
                    && (state.tys[p] != Ty::Vec
                        || vec_kind(c, nodes, &state.tys, center) != Kind::Point)
            {
                return Err("`Dilate` needs a numeric factor and an optional point".into());
            }
            let ty = match state.tys[s] {
                ty @ (Ty::List(_) | Ty::Bool) => ty,
                Ty::Num(_) => Ty::Num(0),
                _ => return Err("expected a list or a function".into()),
            };
            let (pd, pc, pv) = if center == NONE {
                (0, 0, 0)
            } else {
                (state.depth[p], state.cost[p], state.vars[p])
            };
            (
                ty,
                1 + state.depth[s].max(state.depth[f]).max(pd),
                state.cost[s]
                    .saturating_add(state.cost[f])
                    .saturating_add(pc)
                    .saturating_add(1),
                state.vars[s]
                    .saturating_add(state.vars[f])
                    .saturating_add(pv),
            )
        }
        Node::Local(depth, elem) => (state.local(depth, elem)?, 1, 1, 0),
        Node::ObjectDefined(_) => (Ty::Bool, 1, 1, 0),
        Node::Defined(a) => {
            let a = a as usize;
            (
                Ty::Bool,
                1 + state.depth[a],
                1u64.saturating_add(state.cost[a]),
                state.vars[a],
            )
        }
        Node::Filter {
            keep,
            list,
            condition,
            ..
        } => {
            let (l, cond) = (list as usize, condition as usize);
            let Ty::List(elem) = state.tys[l] else {
                return Err("the filter needs a list".into());
            };
            if state.tys[cond] != Ty::Bool {
                return Err("the filter needs a boolean condition".into());
            }
            let ty = if keep { Ty::List(elem) } else { Ty::Num(0) };
            (
                ty,
                1 + state.depth[l].max(state.depth[cond]),
                state.cost[l]
                    .saturating_add(state.cost[cond])
                    .saturating_add(1),
                state.vars[l].saturating_add(state.vars[cond]),
            )
        }
        Node::Vector(a) => {
            let a = a as usize;
            (
                Ty::Vec,
                1 + state.depth[a],
                state.cost[a].saturating_add(1),
                state.vars[a],
            )
        }
        Node::Item(a) => {
            let a = a as usize;
            (state.tys[a], state.depth[a], state.cost[a], state.vars[a])
        }
        Node::ItemObject(_) => (Ty::Object, 1, 1, 0),
        Node::List(first, len) => {
            let mut elem = Elem::Empty;
            let (mut d, mut k, mut v) = (0u32, 1u64, 0u64);
            for item in first..first.saturating_add(len) {
                let item = item as usize;
                elem = join_elem(elem, elem_of(state.tys[item]));
                d = d.max(state.depth[item]);
                k = k.saturating_add(state.cost[item]);
                v = v.saturating_add(state.vars[item]);
            }
            (Ty::List(elem), 1 + d, k, v)
        }
        Node::ListOp(op, l, n) => {
            if op == ListOp::Normalize && !normalize_points_fit(c, nodes, &state.tys, l) {
                return Err("Normalize cannot read a mixed list as points".into());
            }
            let elem = match state.tys[l as usize] {
                Ty::List(e) => e,
                _ => {
                    // `First(locus, n)`: the points of the locus or of the
                    // freehand function (`AlgoFirstLocus`)
                    if matches!(op, ListOp::First | ListOp::Last)
                        && matches!(nodes[l as usize], Node::Obj(id)
                            if c.kind(id) == Kind::Locus
                                || crate::algo::calculus::freehand_list(c, id).is_some())
                        && (n == NONE || matches!(state.tys[n as usize], Ty::Num(_) | Ty::Bool))
                    {
                        return Ok(assign(
                            state,
                            i as u32,
                            Ty::List(Elem::Vec),
                            1 + if n == NONE {
                                0
                            } else {
                                state.depth[n as usize]
                            },
                            1u64 + if n == NONE { 0 } else { state.cost[n as usize] },
                            if n == NONE { 0 } else { state.vars[n as usize] },
                        ));
                    }
                    return Err("the command needs a list".into());
                }
            };
            let object = commands::takes_object(op);
            let second_list = commands::second_list(op);
            if n != NONE
                && !object
                && !second_list
                && !matches!(state.tys[n as usize], Ty::Num(_) | Ty::Bool)
            {
                return Err("the index must be a number".into());
            }
            // a list of numbers and booleans has mixed elements
            let numbers = matches!(elem, Elem::Num | Elem::Bool | Elem::Empty | Elem::Any);
            let ty = commands::list_type(op, elem, numbers, nodes, &state.tys[..i], l, n)?;
            let (nd, nc, nv) = if n == NONE {
                (0, 0, 0)
            } else {
                (
                    state.depth[n as usize],
                    state.cost[n as usize],
                    state.vars[n as usize],
                )
            };
            (
                ty,
                1 + state.depth[l as usize].max(nd),
                1u64.saturating_add(state.cost[l as usize])
                    .saturating_add(nc),
                state.vars[l as usize].saturating_add(nv),
            )
        }
        Node::Range(..) | Node::Seq(..) => {
            let (args, elem): (&[u32], Elem) = match nodes[i] {
                Node::Range(a, b, c2) => (&[a, b, c2], Elem::Num),
                Node::Seq(e, a, b, c2, _) => (&[a, b, c2, e], elem_of(state.tys[e as usize])),
                _ => unreachable!("matched above"),
            };
            let (mut d, mut k, mut v) = (0u32, 1u64, 0u64);
            for (j, &a) in args.iter().enumerate() {
                if a == NONE {
                    continue;
                }
                let a = a as usize;
                if j < 3 && !matches!(state.tys[a], Ty::Num(_) | Ty::Bool) {
                    return Err("the bounds of `Sequence` must be numbers".into());
                }
                d = d.max(state.depth[a]);
                k = k.saturating_add(state.cost[a]);
                v = v.saturating_add(state.vars[a]);
            }
            (Ty::List(elem), 1 + d, k, v)
        }
        Node::Call(CallOp::Zip, first, len) => {
            // the variables of the `Zip` take the elements of their lists;
            // the expression, parsed before them, is typed again with every
            // variable bound, as `resArgsForZip` resolves it last
            let expression = match nodes[first as usize] {
                Node::Item(e) => e,
                _ => return Err("the expression of a `Zip` is missing".into()),
            };
            // the bindings of this `Zip` only: they are popped again once
            // its expression is typed, so a `Zip` never retypes the
            // variable of a later `Sequence` of the same depth
            let bound = state.zip_locals.len();
            for k in 0..(len - 1) / 2 {
                let local = nodes[(first + 1 + 2 * k) as usize];
                let list = match nodes[(first + 2 + 2 * k) as usize] {
                    Node::Item(a) => a,
                    _ => return Err("the list of a `Zip` variable is missing".into()),
                };
                if let (Node::Local(d, _), Ty::List(e)) = (local, state.tys[list as usize]) {
                    let elem = ty_of(e).unwrap_or(Ty::Num(0));
                    state.zip_locals.push((d, elem));
                }
            }
            // the expressions of `Zip` calls nested in this one are typed
            // by their own arms, with their own variables bound
            let block = subtree_min(nodes, expression);
            let mut nested: Vec<(u32, u32)> = Vec::new();
            for j in block..=first {
                if let Node::Call(CallOp::Zip, inner_first, _) = nodes[j as usize] {
                    if let Node::Item(inner_expression) = nodes[inner_first as usize] {
                        nested.push((subtree_min(nodes, inner_expression), inner_expression));
                    }
                }
            }
            // the whole expression is typed again, now with its variables
            // bound (`check` leaves it out of its own walk)
            for j in block..=first {
                if (j as usize) < i && !nested.iter().any(|&(lo, hi)| lo <= j && j <= hi) {
                    type_node(c, nodes, state, j)?;
                }
            }
            state.zip_locals.truncate(bound);
            let tys = &state.tys[..i];
            let (mut md, mut mk, mut mv) = (0u32, 1u64, 0u64);
            for arg in first..first.saturating_add(len) {
                let arg = arg as usize;
                if arg < tys.len() {
                    md = md.max(state.depth[arg]);
                    mk = mk.saturating_add(state.cost[arg]);
                    mv = mv.saturating_add(state.vars[arg]);
                }
            }
            (
                Ty::List(elem_of(state.tys[expression as usize])),
                1 + md,
                mk,
                mv,
            )
        }
        Node::Call(op, first, len) => {
            let ty = commands::call_type(op, c, nodes, &state.tys[..i], first, len)?;
            let (mut md, mut mk, mut mv) = (0u32, 1u64, 0u64);
            for arg in first..first.saturating_add(len) {
                let arg = arg as usize;
                md = md.max(state.depth[arg]);
                mk = mk.saturating_add(state.cost[arg]);
                mv = mv.saturating_add(state.vars[arg]);
            }
            (ty, 1 + md, mk, mv)
        }
    };
    state.tys[i] = ty;
    state.depth[i] = d;
    state.cost[i] = k;
    state.vars[i] = v;
    Ok((ty, d, k, v))
}

/// Assigns an already computed type and returns it (a typing that does not
/// read the state any further).
fn assign(state: &mut State, i: u32, ty: Ty, d: u32, k: u64, v: u64) -> (Ty, u32, u64, u64) {
    state.tys[i as usize] = ty;
    state.depth[i as usize] = d;
    state.cost[i as usize] = k;
    state.vars[i as usize] = v;
    (ty, d, k, v)
}

fn object_type(c: &Construction, id: Id) -> Result<Ty, String> {
    match (c.kind(id), c.def(id)) {
        (Kind::List, Def::Expr(d)) => Ok(super::entry(c, d).root_ty()),
        (Kind::List, Def::Chart(_) | Def::Prove(_)) => Ok(Ty::List(Elem::Any)),
        // a locus is the argument of `First(locus, n)` (`AlgoFirstLocus`);
        // anywhere else it reads as a plain number, which no operation
        // makes sense of and evaluates to the undefined number
        (Kind::Locus, _) => Ok(Ty::Num(0)),
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

/// The reference reads every entry as a point when the first entry is a
/// point or the last entry is a point after a numeric scan.
fn normalize_points_fit(c: &Construction, nodes: &[Node], types: &[Ty], list: u32) -> bool {
    let fits = |len: usize, first_numeric: bool, is_point: &dyn Fn(usize) -> bool| {
        len == 0 || !(is_point(0) || first_numeric && is_point(len - 1)) || (0..len).all(is_point)
    };
    match nodes[list as usize] {
        Node::List(start, len) if len != 0 => {
            let is_point = |i: usize| {
                let item = start + i as u32;
                types[item as usize] == Ty::Vec && vec_kind(c, nodes, types, item) == Kind::Point
            };
            fits(
                len as usize,
                matches!(types[start as usize], Ty::Num(_)),
                &is_point,
            )
        }
        Node::Obj(id) => c.elements(id).is_none_or(|items| {
            let first_numeric =
                matches!(items.get(0), Some(super::Element::Value(kind, _)) if kind.is_number());
            fits(items.len(), first_numeric, &|i| {
                items.raw_point(i).is_some()
            })
        }),
        _ => true,
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
        (Un::Abs | Un::Length, Ty::Vec) => Ok(Ty::Num(0)),
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
    if matches!(a, Ty::Object | Ty::Function) || matches!(b, Ty::Object | Ty::Function) {
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
        (Bin::Pow | Bin::RootPow, Num(x), Num(y)) => Ok(Num(if y > 0 { 0 } else { x })),
        (Bin::Pow | Bin::RootPow, Vec, Num(_)) => Ok(Num(0)),
        (Bin::Round2, Num(x), Num(_)) => Ok(Num(x)),
        (Bin::Atan2 | Bin::LogB | Bin::FoldCount, Num(_), Num(_)) => Ok(Num(0)),
        (op, Num(_), Num(_)) if op.is_command() => Ok(Num(0)),
        (op, _, _) if op.is_command() => Err("the command takes two numbers".into()),
        _ => complex(),
    }
}

/// Widen through the alternatives of one call until the first incompatible
/// assignment. A nested `If` expression is a single alternative.
fn conditional_type(nodes: &[Node], types: &[Ty], first: usize, mut next: usize) -> Ty {
    let mut result = types[first];
    for _ in 0..super::commands::logical::MAX_IF_ALTERNATIVES {
        let (branch, tail) = match nodes[next] {
            Node::If(IfMode::AlgoNext, _, branch, rest) => (branch as usize, rest),
            _ => (next, NONE),
        };
        result = match (result, types[branch]) {
            (Ty::Bool | Ty::Num(1), t @ Ty::Num(n)) if n != 1 => t,
            (Ty::Bool | Ty::Num(_), Ty::Vec) => Ty::Vec,
            (Ty::List(a), Ty::List(b)) => Ty::List(join_elem(a, b)),
            (a, b) if a == b => a,
            (Ty::Num(a), Ty::Num(b)) if a != 1 && b != 1 => Ty::Num(b),
            _ => break,
        };
        if tail == NONE {
            break;
        }
        next = tail as usize;
    }
    result
}

/// Combine the numeric branches of a conditional function.
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

#[cfg(test)]
mod number_valued_object_tests {
    #[test]
    fn comparisons_of_number_valued_objects_stay_refused() {
        let base = "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\nD = (4, 3)\n\
            p = Polygon(A, B, C)\nq = Polygon(A, B, D)\nr = PolyLine(A, B, C)\n";
        crate::script::load(base).unwrap();
        for line in [
            "e = p == q",
            "e = p != q",
            "e = p == 6",
            "e = r == 6",
            "e = p == r",
        ] {
            assert!(
                crate::script::load(&format!("{base}{line}")).is_err(),
                "`{line}` should be refused"
            );
        }
        assert!(crate::script::load(&format!("{base}f = Line(A, B)\ne = f != 6")).is_ok());
    }
}
