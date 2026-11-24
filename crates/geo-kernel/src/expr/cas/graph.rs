//! A line or conic used as a function by CmdSimplify.

use super::functional::{Function, FunctionResult};
use super::{input, Op, Poly, Quotient, Rat, Work};
use crate::expr::{children, equation, eval, Entry, Node, NONE};
use crate::{Construction, Id, Kind};
use std::sync::Arc;

/// Bound the number of native rewrites an expression can request per update.
pub(in crate::expr) const MAX_NESTED_REWRITES: usize = 8;

pub(super) fn is_graph(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Line | Kind::Segment | Kind::Ray | Kind::Conic | Kind::Circle
    )
}

pub(in crate::expr) fn source(c: &Construction, nodes: &[Node], a: u32) -> Option<Id> {
    let Node::Obj(id) = nodes[a as usize] else {
        return None;
    };
    is_graph(c.kind(id)).then_some(id)
}

/// Preserve the entered expression for an equation of the form y = f(x).
pub(super) fn right_side(c: &Construction, id: Id) -> Option<&Entry> {
    let left = equation::side_entry(c, id, true)?;
    let right = equation::side_entry(c, id, false)?;
    (left.nodes[left.root as usize] == Node::VarY && !right.nodes.contains(&Node::VarY))
        .then_some(right)
}

/// The polynomial a*x² + b*x + c, and the coefficient of y.
pub(super) fn coefficients(c: &Construction, id: Id, tolerance: f64) -> Option<[f64; 4]> {
    if let Some((a, b, constant)) = c.raw_carrier(id) {
        return Some([0.0, a, constant, b]);
    }
    let [xx, yy, constant, xy, x, y] = c.conic(id)?.matrix;
    let zero = |v: f64| v == 0.0 || v.abs() < tolerance;
    (c.value(id).is_defined() && zero(xy) && zero(yy)).then_some([xx, 2.0 * x, constant, 2.0 * y])
}

fn exact(c: &Construction, id: Id) -> Option<FunctionResult> {
    let quotient = if let Some(right) = right_side(c, id) {
        let allowance = eval::Lists::new(Vec::new());
        let charge = |n| allowance.charge(n as u64);
        let work = &mut Work::new(&charge);
        let value = input::read(c, right, right.root, &input::variables(), work)?;
        value.normalized(work)?
    } else {
        let [xx, x, constant, y] = coefficients(c, id, 0.0)?;
        let divisor = Rat::from_value(y)?.neg()?;
        let coefficient = |v| Rat::from_value(v)?.div(divisor);
        Quotient::polynomial(Poly::dense(&[
            coefficient(constant)?,
            coefficient(x)?,
            coefficient(xx)?,
        ]))
    };
    Some(FunctionResult::Exact(Function::new(quotient)))
}

pub(in crate::expr) fn admits_nested(c: &Construction, id: Id) -> bool {
    #[cfg(feature = "giac")]
    if super::giac_admits() {
        return matches!(c.kind(id), Kind::Line | Kind::Segment | Kind::Ray)
            || right_side(c, id).is_some()
            || coefficients(c, id, crate::tol::STANDARD_PRECISION)
                .is_some_and(|[_, _, _, divisor]| divisor.is_finite() && divisor != 0.0);
    }
    exact(c, id).is_some()
}

pub(in crate::expr) type Rewrites = Option<Arc<[(u32, Option<FunctionResult>)]>>;

fn rewrite(c: &Construction, e: &Entry, i: u32) -> Option<FunctionResult> {
    let Node::Cas(Op::Simplify, a, NONE) = e.nodes[i as usize] else {
        return None;
    };
    let id = source(c, &e.nodes, a)?;
    if !eval::reserve_work(1) {
        return None;
    }
    #[cfg(feature = "giac")]
    if let Some(result) = super::giac::rewrite::function(c, e, Op::Simplify, a) {
        return result;
    }
    exact(c, id)
}

/// Cache the function for each reachable nested graph rewrite. Sampling
/// the surrounding expression only evaluates the cached function.
pub(in crate::expr) fn initial(c: &Construction, e: &Entry) -> Rewrites {
    let root = e.body_root();
    let is_nested = |i, node| match node {
        Node::Cas(Op::Simplify, a, NONE) if i != root => source(c, &e.nodes, a).is_some(),
        _ => false,
    };
    if !e
        .nodes
        .iter()
        .enumerate()
        .any(|(i, &node)| is_nested(i as u32, node))
    {
        return None;
    }
    let mut reachable = vec![false; e.nodes.len()];
    reachable[e.root as usize] = true;
    let mut sites = Vec::new();
    for (i, &node) in e.nodes.iter().enumerate().rev() {
        if reachable[i] {
            for child in children(&node) {
                reachable[child as usize] = true;
            }
            if is_nested(i as u32, node) {
                sites.push(i as u32);
            }
        }
    }
    Some(sites.into_iter().map(|i| (i, rewrite(c, e, i))).collect())
}

pub(in crate::expr) fn refresh(c: &Construction, e: &Entry) -> Rewrites {
    Some(
        e.graph_rewrites
            .as_ref()?
            .iter()
            .map(|&(i, _)| (i, rewrite(c, e, i)))
            .collect(),
    )
}
