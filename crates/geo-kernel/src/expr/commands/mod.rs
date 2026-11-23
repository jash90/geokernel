//! The commands that expressions compute themselves, one file per group
//! of the command matrix (the `group` column of `docs/command-matrix.csv`).
//!
//! Each group file owns the names of its commands, how they are parsed,
//! their types and their evaluation; the functions here only ask the
//! groups in turn, and each group answers `None` for a command that is not
//! its own. A new group adds its `mod` line and one line to each function
//! here that its commands need.

mod algebra;
pub(super) use algebra::integers::{factor as integer_factors, prime};

pub(crate) use algebra::{extrema::brent_minimum, polynomial};
pub(super) mod financial;
pub(super) mod function_analysis;
mod geogebra;
mod geometry;
mod list;
pub(super) mod logical;
pub(crate) use logical::relation;
mod statistics;
pub(super) mod text;

use super::eval::{Ctx, Val};
use super::parse::Parser;
use super::{Bin, Elem, ListOp, Node, Ty};

pub(in crate::expr) use list::{call_type, call_value, second_list};
pub(in crate::expr) use text::call_node as text_call;

/// Whether `name` is a command that expressions compute themselves (the
/// two-number commands are built-in names instead, see
/// [`binary_command`]).
pub(super) fn is_command(name: &str) -> bool {
    [
        algebra::NAMES,
        super::cas::NAMES,
        function_analysis::NAMES,
        logical::NAMES,
        list::NAMES,
        statistics::NAMES,
        geometry::NAMES,
        text::NAMES,
        financial::NAMES,
        geogebra::NAMES,
    ]
    .iter()
    .any(|names| names.contains(&name))
}

/// The call `text(...)` of a command that a group parses itself: its node,
/// or `None`, with nothing read, when no group takes it (the two-number
/// commands and the commands of the script parser are tried next).
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if let Some(node) = p.cas_command(text)? {
        return Ok(Some(node));
    }
    if let Some(node) = function_analysis::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = text::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = logical::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = list::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = statistics::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = financial::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = geometry::parse(p, text)? {
        return Ok(Some(node));
    }
    if let Some(node) = geogebra::parse(p, text)? {
        return Ok(Some(node));
    }
    algebra::parse(p, text)
}

/// The command of two numbers named `name`.
pub(super) fn binary_command(name: &str) -> Option<Bin> {
    algebra::binary_command(name)
}

/// Whether `op` is a command of two numbers.
pub(super) fn is_binary_command(op: Bin) -> bool {
    algebra::is_binary_command(op)
}

/// The command of two numbers `op` of `a` and `b`.
pub(super) fn binary(op: Bin, a: f64, b: f64) -> f64 {
    algebra::binary(op, a, b)
}

/// Whether the result of the list command `op` is a list.
pub(super) fn makes_list(op: ListOp) -> bool {
    matches!(op, ListOp::Normalize | ListOp::Svd)
        || geometry::makes_list(op)
        || list::makes_list(op)
}

/// Whether the second operand of the list command `op` is an object rather
/// than a number.
pub(super) fn takes_object(op: ListOp) -> bool {
    matches!(
        op,
        ListOp::Dot | ListOp::Cross | ListOp::ApplyMatrix | ListOp::Min | ListOp::Max
    ) || list::takes_object(op)
}

/// The type of the list command `op` of the list `l`, whose elements are
/// `elem` (`numbers` when they may all be numbers), and of `n`.
pub(super) fn list_type(
    op: ListOp,
    elem: Elem,
    numbers: bool,
    nodes: &[Node],
    tys: &[Ty],
    l: u32,
    n: u32,
) -> Result<Ty, String> {
    if op == ListOp::Normalize {
        return Ok(Ty::List(elem));
    }
    if op == ListOp::Svd {
        return Ok(Ty::List(Elem::List));
    }
    let groups = [
        list::list_type,
        statistics::list_type,
        algebra::list_type,
        geometry::list_type,
    ];
    for group in groups {
        if let Some(ty) = group(op, elem, numbers, nodes, tys, l, n)? {
            return Ok(ty);
        }
    }
    unreachable!("no group types {op:?}")
}

/// The value of the list command `op` at node `i` that gives a number, a
/// point or a boolean.
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Val {
    list::list_value(ctx, i, op, l, n)
        .or_else(|| statistics::list_value(ctx, i, op, l, n))
        .or_else(|| algebra::list_value(ctx, i, op, l, n))
        .or_else(|| geometry::list_value(ctx, i, op, l, n))
        .unwrap_or_else(|| unreachable!("no group evaluates {op:?}"))
}

/// The list that the list command `op` whose result is a list builds (see
/// [`makes_list`]).
pub(super) use geometry::map_list;

pub(super) fn list_fn<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<(u32, u32)> {
    match op {
        ListOp::Normalize => function_analysis::normalize(ctx, l),
        ListOp::Svd => function_analysis::svd(ctx, l),
        _ if geometry::makes_list(op) => geometry::list_fn(ctx, op, l, n),
        _ => ctx.list_fn(op, l, n),
    }
}
