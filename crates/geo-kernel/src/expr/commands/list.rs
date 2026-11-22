// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSequence.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSequence.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSequenceRange.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoListElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoReverse.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFirst.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLast.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSort.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAppend.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoin.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdTake.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTake.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRemoveUndefined.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRemoveUndefined.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdIndexOf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIndexOf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdFlatten.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoFlatten.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdInsert.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoInsert.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdProduct.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSum.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoStats1D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFoldExpression.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/CmdFrequency.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoFrequency.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/FrequencyGgb.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/CmdClasses.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoClasses.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdUnique.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoUnique.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdZip.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoZip.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdIntersection.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersection.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdUnion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoUnion.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRemove.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRemove.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPointList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPointList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOneListFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/CmdOrdinalRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoOrdinalRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/CmdTiedRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoTiedRank.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoFirstLocus.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdElement.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSort.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdFirst.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLast.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdReverse.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The list group of the command matrix: `Sequence`, `Element` (and
//! `l(n)`), `Product`, `Reverse`, `First`, `Last`, `Sort`, `Append`,
//! `Join`, `Take`, `RemoveUndefined`, `IndexOf`, `Flatten`, `Insert`,
//! `Frequency`, `Unique`, `Classes`, `Zip`, `Intersection`, `Union`,
//! `Remove`, `PointList`, `OrdinalRank` and `TiedRank`.

use crate::expr::eval::{count, number_of, slot_val, work, Ctx, Local, Val, OBJECT_SHIFT};
use crate::expr::lex::Tok;
use crate::expr::list::Slot;
use crate::expr::math;
use crate::expr::parse::{list_arity, list_form, Parser};
use crate::expr::types::{elem_of, join_elem, of_numbers, ty_of};
use crate::expr::Bin;
use crate::expr::{CallOp, Elem, ListOp, Node, Ty, NONE};
use crate::kernel::{Construction, Kind, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &[
    "Sequence",
    "Element",
    "Product",
    "Reverse",
    "First",
    "Last",
    "Sort",
    "Append",
    "Join",
    "Take",
    "RemoveUndefined",
    "IndexOf",
    "Flatten",
    "Insert",
    "Frequency",
    "Unique",
    "Classes",
    "Zip",
    "Intersection",
    "Union",
    "Remove",
    "PointList",
    "OrdinalRank",
    "TiedRank",
];

/// `Sequence` and `Element`, and the other commands of a list (of anything
/// else they are the commands of the script parser).
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if let Some(op) = one_list_command(text) {
        return p.list_command(|p, args, lists| {
            wrapped(p, text, args, lists).map(|l| l.map(|l| Node::ListOp(op, l, NONE)))
        });
    }
    let (op, count) = match text {
        "Sequence" => return p.sequence().map(Some),
        "Element" => {
            return match p.arguments()?.as_slice() {
                [l, n] => Ok(Some(p.push(Node::ListOp(ListOp::Element, *l, *n)))),
                // more indices walk into a list of lists (`AlgoListElement`)
                [l, n, rest @ ..] if !rest.is_empty() => {
                    let mut node = p.push(Node::ListOp(ListOp::Element, *l, *n));
                    for &k in rest {
                        node = p.push(Node::ListOp(ListOp::Element, node, k));
                    }
                    Ok(Some(node))
                }
                _ => Err("`Element` takes a list and one index".into()),
            };
        }
        "Append" => return p.list_command(|_, args, lists| append(text, args, lists)),
        "Join" => return p.list_command(|p, args, lists| join(p, text, args, lists)),
        "Product" => return product(p, text),
        "First" | "Last" => return first_last(p, text, ListOp::First, text == "Last"),
        "Sort" => return sort(p, text),
        "Take" => return take(p, text),
        "IndexOf" => return index_of(p, text),
        "Insert" => return insert(p, text),
        "Classes" => return classes(p, text),
        "Frequency" => return frequency(p, text),
        "Zip" => return p.zip(),
        "Union" | "Intersection" | "Remove" => {
            let op = match text {
                "Union" => ListOp::Union,
                "Intersection" => ListOp::Intersect,
                _ => ListOp::Remove,
            };
            return p.list_command(|_, args, lists| match (args, lists) {
                (&[a, b], [true, true]) => Ok(Some(Node::ListOp(op, a, b))),
                (_, [false, ..]) => Ok(None),
                _ => Err(list_arity(text, args.len())),
            });
        }
        "Reverse" => (ListOp::Reverse, false),
        _ => return Ok(None),
    };
    p.list_command(|_, args, lists| list_form(text, op, count, args, lists))
}

/// `Kernel.STANDARD_PRECISION`, the window `DoubleUtil.isEqual` reads two
/// numbers in.
const STANDARD_PRECISION: f64 = 1e-8;

/// `DoubleUtil.isEqual`: equal, or within `STANDARD_PRECISION` of each
/// other.
fn near(a: f64, b: f64) -> bool {
    a == b || (a - STANDARD_PRECISION..=a + STANDARD_PRECISION).contains(&b)
}

/// A number as the key of an ordered table (`Frequency`'s `TreeMap`): the
/// total order of `total_cmp`; -0 and 0 never both live in one.
#[derive(Clone, Copy)]
struct Known(f64);

impl PartialEq for Known {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Known {}

impl PartialOrd for Known {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Known {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The window a bucket lookup starts from, widened for the rounding of its
/// bounds; `near` picks among the few buckets inside (they are never near
/// each other).
fn bucket_window(v: f64) -> std::ops::RangeInclusive<Known> {
    Known(v - 3.0 * STANDARD_PRECISION)..=Known(v + 3.0 * STANDARD_PRECISION)
}

/// `Frequency.getCount(v)`: the count of the bucket equal to `v` within
/// `STANDARD_PRECISION`.
fn table_count(table: &BTreeMap<Known, u64>, v: f64) -> u64 {
    table
        .range(bucket_window(v))
        .find(|&(kept, _)| near(kept.0, v))
        .map_or(0, |(_, &count)| count)
}

/// `Frequency.getCumFreq(v)`: the counts of every bucket below `v`, and of
/// the one equal to it within `STANDARD_PRECISION`, over the running
/// sums of the table (a lookup each, where GeoGebra walks every bucket
/// below `v` again).
struct Cumulative<'t> {
    keys: Vec<Known>,
    /// `sums[i]` holds the counts of `keys[0..=i]`.
    sums: Vec<u64>,
    table: &'t BTreeMap<Known, u64>,
}

impl<'t> Cumulative<'t> {
    fn new(table: &'t BTreeMap<Known, u64>) -> Self {
        let mut keys = Vec::with_capacity(table.len());
        let mut sums = Vec::with_capacity(table.len());
        let mut total = 0u64;
        for (k, &count) in table {
            total += count;
            keys.push(*k);
            sums.push(total);
        }
        Cumulative { keys, sums, table }
    }

    /// The buckets the window of `v` starts at.
    fn at(&self, v: f64) -> usize {
        self.keys
            .partition_point(|&k| k.0 < v - 3.0 * STANDARD_PRECISION)
    }

    /// The counts of every bucket below the window of `v`.
    fn below(&self, at: usize) -> u64 {
        if at == 0 {
            0
        } else {
            self.sums[at - 1]
        }
    }

    /// The counts of every bucket below `v` and of the one equal to it
    /// (the buckets of one table are never near each other, so the
    /// candidates in the window are few).
    fn up_to(&self, v: f64) -> u64 {
        let at = self.at(v);
        let mut sum = self.below(at);
        for (&Known(k), &count) in self.table.range(bucket_window(v)) {
            if k < v || near(k, v) {
                sum += count;
            }
        }
        sum
    }
}

/// The bucket table of `Frequency` (`AlgoFrequency`): numbers with the
/// rounding comparator of `FrequencyGgb`, or exact strings in natural
/// order.
enum Table {
    Numbers(BTreeMap<Known, u64>, u32),
    Texts(BTreeMap<Vec<u16>, u64>),
}

/// The commands of one list argument (`CmdOneListFunction`); the loose
/// number and point arguments of `Reverse`, `PointList`, `OrdinalRank`
/// and `TiedRank` are wrapped in a list.
fn one_list_command(text: &str) -> Option<ListOp> {
    match text {
        "Reverse" => Some(ListOp::Reverse),
        "PointList" => Some(ListOp::PointList),
        "OrdinalRank" => Some(ListOp::OrdinalRank),
        "TiedRank" => Some(ListOp::TiedRank),
        "RemoveUndefined" => Some(ListOp::RemoveUndefined),
        "Unique" => Some(ListOp::Unique),
        "Flatten" => Some(ListOp::Flatten),
        _ => None,
    }
}

/// The list of a command of one list: the argument itself, or the list of
/// the arguments when every one of them is a number or a point or vector
/// (`wrapInList`); `None` when they are not.
fn wrapped(
    p: &mut Parser,
    text: &str,
    args: &[u32],
    lists: &[bool],
) -> Result<Option<u32>, String> {
    match (args, lists) {
        (&[l], [true]) => Ok(Some(l)),
        (_, [true, ..]) => Err(list_arity(text, args.len())),
        (_, [false, ..])
            if !matches!(
                text,
                "Reverse" | "PointList" | "OrdinalRank" | "TiedRank" | "Sort"
            ) || !p.are_numbers(args) && !p.are_points(args) =>
        {
            Ok(None)
        }
        _ => {
            let first = p.nodes.len() as u32;
            for &a in args {
                p.push(Node::Item(a));
            }
            Ok(Some(p.push(Node::List(first, args.len() as u32))))
        }
    }
}

/// `Product`: the forms of a list, of a list and a list of frequencies
/// (`AlgoStats1D`) and of an expression (`CmdSum.processSymb`).
fn product(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if let Some(node) = p.fold(text)? {
        return Ok(Some(node));
    }
    let saved = p.saved();
    let args = p.arguments()?;
    let lists: Vec<bool> = args.iter().map(|&a| p.is_list(a)).collect();
    let node = match (args.as_slice(), lists.as_slice()) {
        (_, [false, ..]) => {
            p.reset(saved);
            return Ok(None);
        }
        (&[l], _) => Node::ListOp(ListOp::Product, l, NONE),
        (&[l, n], _) => Node::ListOp(ListOp::Product, l, n),
        _ => return Err(list_arity(text, args.len())),
    };
    Ok(Some(p.push(node)))
}

/// `First` and `Last` of a list, and `First(locus, n)`
/// (`AlgoFirstLocus`, `CmdFirst`).
fn first_last(
    p: &mut Parser,
    text: &str,
    first: ListOp,
    last: bool,
) -> Result<Option<u32>, String> {
    let op = if last { ListOp::Last } else { first };
    if let Some(node) = p.list_command(|_, args, lists| list_form(text, op, true, args, lists))? {
        return Ok(Some(node));
    }
    let saved = p.saved();
    match p.arguments()?.as_slice() {
        [l, n] if p.is_locus(*l) && p.is_number(*n) => Ok(Some(p.push(Node::ListOp(op, *l, *n)))),
        _ => {
            p.reset(saved);
            Ok(None)
        }
    }
}

/// `Sort(list)`, `Sort(values, keys)` and `Sort(point, point, ...)`
/// (`CmdSort`).
fn sort(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    p.list_command(|p, args, lists| {
        let node = match (args, lists) {
            (&[v, k], [true, true]) => Node::ListOp(ListOp::Sort, v, k),
            (&[k], [true]) => Node::ListOp(ListOp::Sort, k, NONE),
            // two points are the wrapped form in GeoGebra (its `case 2`
            // falls through), more points always are
            _ if p.are_points(args) => {
                return wrapped(p, text, args, lists)
                    .map(|l| l.map(|l| Node::ListOp(ListOp::Sort, l, NONE)))
            }
            (_, [false, ..]) => return Ok(None),
            _ => return Err(list_arity(text, args.len())),
        };
        Ok(Some(node))
    })
}

/// `Take(list, from[, to])` (`CmdTake`).
fn take(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let saved = p.saved();
    let args = p.arguments()?;
    if !matches!(args.len(), 2 | 3) {
        return Err(format!("`{text}` takes two or three arguments"));
    }
    if !p.is_list(args[0]) {
        p.reset(saved);
        return Ok(None);
    }
    if !args[1..].iter().all(|&a| p.is_number(a)) {
        return Err(format!("`{text}` takes numbers as its bounds"));
    }
    let first = p.call_run(&args);
    Ok(Some(p.push(Node::Call(
        CallOp::Take,
        first,
        args.len() as u32,
    ))))
}

/// `IndexOf(object, list[, start])` (`CmdIndexOf`).
fn index_of(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let args = p.arguments()?;
    if !matches!(args.len(), 2 | 3) {
        return Err(format!("`{text}` takes two or three arguments"));
    }
    if !p.is_list(args[1]) {
        return Err(format!("`{text}` searches in a list"));
    }
    if args.len() == 3 && !p.is_number(args[2]) {
        return Err(format!("`{text}` starts at a number"));
    }
    let first = p.call_run(&args);
    Ok(Some(p.push(Node::Call(
        CallOp::IndexOf,
        first,
        args.len() as u32,
    ))))
}

/// `Insert(object, list, position)` (`CmdInsert`).
fn insert(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let args = p.arguments()?;
    if args.len() != 3 {
        return Err(format!("`{text}` takes three arguments"));
    }
    if !p.is_list(args[1]) || !p.is_number(args[2]) {
        return Err(format!("`{text}` inserts in a list at a number"));
    }
    let first = p.call_run(&args);
    Ok(Some(p.push(Node::Call(
        CallOp::Insert,
        first,
        args.len() as u32,
    ))))
}

/// `Classes(list, classes | start, width)` (`CmdClasses`).
fn classes(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let args = p.arguments()?;
    if !matches!(args.len(), 2 | 3) {
        return Err(format!("`{text}` takes two or three arguments"));
    }
    if !p.is_list(args[0]) || !args[1..].iter().all(|&a| p.is_number(a)) {
        return Err(format!("`{text}` cuts a list at numbers"));
    }
    let first = p.call_run(&args);
    Ok(Some(p.push(Node::Call(
        CallOp::Classes,
        first,
        args.len() as u32,
    ))))
}

/// `Frequency` in its eight forms (`CmdFrequency`): the boolean and number
/// positions tell them apart.
fn frequency(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let saved = p.saved();
    let args = p.arguments()?;
    // (boolean positions, list positions, trailing numbers) per form
    const FORMS: [&[usize]; 8] = [&[], &[0], &[2], &[0], &[3], &[0], &[3, 4], &[0, 4]];
    let lists: Vec<bool> = args.iter().map(|&a| p.is_list(a)).collect();
    let bools: Vec<bool> = args.iter().map(|&a| p.is_bool(a)).collect();
    let nums: Vec<bool> = args.iter().map(|&a| p.is_number(a)).collect();
    let is_bool = |i: usize| bools.get(i).copied() == Some(true);
    let is_list = |i: usize| lists.get(i).copied() == Some(true);
    let is_num = |i: usize| nums.get(i).copied() == Some(true);
    let takes = |bs: &[usize]| match args.len() {
        1 => bs.is_empty() && is_list(0),
        2 => bs == [0] && is_bool(0) && is_list(1) || bs.is_empty() && is_list(0) && is_list(1),
        3 => {
            bs == [0] && is_bool(0) && is_list(1) && is_list(2)
                || bs.is_empty() && is_list(0) && is_list(1) && is_bool(2)
        }
        4 => {
            bs.is_empty() && is_list(0) && is_list(1) && is_bool(2) && is_num(3)
                || bs == [0] && is_bool(0) && is_list(1) && is_list(2) && is_bool(3)
        }
        5 => bs == [0] && is_bool(0) && is_list(1) && is_list(2) && is_bool(3) && is_num(4),
        _ => false,
    };
    if FORMS.iter().any(|bs| takes(bs)) {
        let first = p.call_run(&args);
        return Ok(Some(p.push(Node::Call(
            CallOp::Frequency,
            first,
            args.len() as u32,
        ))));
    }
    if args.len() >= 2 && args.len() <= 5 && (bools.iter().any(|&b| b) || nums.iter().any(|&n| n)) {
        p.reset(saved);
        return Ok(None);
    }
    Err(format!("`{text}` does not take these arguments"))
}

/// `Append(list, object)` and `Append(object, list)`.
fn append(text: &str, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    match (args, lists) {
        (&[l, x], [true, _]) => Ok(Some(Node::ListOp(ListOp::Append, l, x))),
        (&[x, l], [false, true]) => Ok(Some(Node::ListOp(ListOp::Prepend, l, x))),
        (_, [false, ..]) => Ok(None),
        _ => Err(list_arity(text, args.len())),
    }
}

/// `Join(list of lists)` and `Join(l1, l2, ...)`.
fn join(p: &mut Parser, text: &str, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    match (args, lists) {
        (&[l], [true]) => Ok(Some(Node::ListOp(ListOp::Join, l, NONE))),
        _ if lists.iter().all(|&b| b) => {
            // `CmdJoin`: the list of the arguments
            let first = p.nodes.len() as u32;
            for &a in args {
                p.push(Node::Item(a));
            }
            let list = p.push(Node::List(first, args.len() as u32));
            Ok(Some(Node::ListOp(ListOp::Join, list, NONE)))
        }
        _ => list_form(text, ListOp::Join, false, args, lists),
    }
}

/// Whether the result of `op` is a list.
pub(super) fn makes_list(op: ListOp) -> bool {
    matches!(
        op,
        ListOp::Reverse
            | ListOp::First
            | ListOp::Last
            | ListOp::Sort
            | ListOp::Append
            | ListOp::Prepend
            | ListOp::Join
            | ListOp::RemoveUndefined
            | ListOp::Unique
            | ListOp::Flatten
            | ListOp::PointList
            | ListOp::OrdinalRank
            | ListOp::TiedRank
            | ListOp::Union
            | ListOp::Intersect
            | ListOp::Remove
    )
}

/// Whether the second operand of `op` is an object.
pub(super) fn takes_object(op: ListOp) -> bool {
    matches!(op, ListOp::Append | ListOp::Prepend)
}

/// Whether the second operand of `op` is a list rather than a number
/// (`Sort(values, keys)`, `Product(values, frequencies)`, `Sum(list,
/// frequencies)`, `Union`, `Intersection` and `Remove`).
pub(in crate::expr) fn second_list(op: ListOp) -> bool {
    matches!(
        op,
        ListOp::Sort
            | ListOp::Product
            | ListOp::Sum
            | ListOp::Union
            | ListOp::Intersect
            | ListOp::Remove
    )
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    elem: Elem,
    numbers: bool,
    nodes: &[Node],
    tys: &[Ty],
    l: u32,
    n: u32,
) -> Result<Option<Ty>, String> {
    Ok(Some(match op {
        // `AlgoStats1D` reads numbers
        ListOp::Product if n != NONE && matches!(tys[n as usize], Ty::List(_)) => {
            of_numbers(numbers)?
        }
        ListOp::Product => of_numbers(numbers)?,
        // `Element` of a list whose elements the types cannot tell apart
        // (an `Element` of it, an `If` of lists) is typed as a number:
        // GeoGebra finds what it is when it evaluates the line
        ListOp::Element if matches!(elem, Elem::Any) => Ty::Num(0),
        ListOp::Index | ListOp::Element => ty_of(elem)?,
        ListOp::Reverse | ListOp::First | ListOp::Last => Ty::List(elem),
        // `AlgoSort` outputs the elements of its value list
        ListOp::Sort if n != NONE => match tys[l as usize] {
            Ty::List(e) => Ty::List(e),
            _ => return Err("`Sort` needs two lists".into()),
        },
        ListOp::Sort => Ty::List(elem),
        ListOp::Append | ListOp::Prepend => Ty::List(join_elem(elem, elem_of(tys[n as usize]))),
        ListOp::RemoveUndefined | ListOp::Unique => Ty::List(elem),
        ListOp::Flatten => Ty::List(Elem::Any),
        ListOp::PointList => Ty::List(Elem::Vec),
        // `AlgoOrdinalRank` and `AlgoTiedRank` make an undefined list for
        // elements that are no numbers, but the command still runs
        ListOp::OrdinalRank | ListOp::TiedRank => Ty::List(Elem::Num),
        ListOp::Union | ListOp::Intersect => Ty::List(join_elem(elem, elem_of(tys[n as usize]))),
        ListOp::Remove => Ty::List(elem),
        ListOp::Join => match nodes[l as usize] {
            // the list of the arguments: their elements
            Node::List(first, len) => {
                let mut joined = Elem::Empty;
                for item in first..first + len {
                    match tys[item as usize] {
                        Ty::List(e) => joined = join_elem(joined, e),
                        _ => return Err("`Join` needs lists".into()),
                    }
                }
                Ty::List(joined)
            }
            _ if matches!(elem, Elem::List | Elem::Empty) => Ty::List(Elem::Any),
            _ => return Err("`Join` needs a list of lists".into()),
        },
        _ => return Ok(None),
    }))
}

/// The type of a [`Node::Call`] of this group (see `commands::call_type`):
/// `args` are its `len` arguments from `first` on.
pub(crate) fn call_type(
    op: CallOp,
    _c: &Construction,
    _nodes: &[Node],
    tys: &[Ty],
    first: u32,
    len: u32,
) -> Result<Ty, String> {
    let args = |k: usize| first + k as u32;
    let list_elem = |k: usize| -> Result<Elem, String> {
        match tys[args(k) as usize] {
            Ty::List(e) => Ok(e),
            _ => Err("the command needs a list".into()),
        }
    };
    Ok(match op {
        CallOp::Take => Ty::List(list_elem(0)?),
        CallOp::IndexOf => Ty::Num(0),
        CallOp::Insert => Ty::List(join_elem(list_elem(1)?, elem_of(tys[args(0) as usize]))),
        CallOp::Classes => {
            list_elem(0)?;
            Ty::List(Elem::Num)
        }
        CallOp::Frequency => {
            // the data list: the last argument that is a list
            for k in (0..len as usize).rev() {
                if matches!(tys[args(k) as usize], Ty::List(_)) {
                    list_elem(k)?;
                    return Ok(Ty::List(Elem::Num));
                }
            }
            return Err("`Frequency` needs a list of data".into());
        }
        CallOp::Zip => Ty::List(elem_of(tys[first as usize])),
        // points and vectors fold to a point (`PointNDFold`)
        CallOp::Fold | CallOp::SumFold if tys[first as usize] == Ty::Vec => Ty::Vec,
        CallOp::Fold | CallOp::SumFold => Ty::Num(0),
    })
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Product if n != NONE && ctx.is_list(n) => Val::N(ctx.product_frequencies(l, n)),
        ListOp::Product => Val::N(ctx.stats(op, l, n)),
        // a list: built by `list`
        ListOp::Reverse
        | ListOp::First
        | ListOp::Last
        | ListOp::Sort
        | ListOp::Append
        | ListOp::Prepend
        | ListOp::Join
        | ListOp::RemoveUndefined
        | ListOp::Unique
        | ListOp::Flatten
        | ListOp::PointList
        | ListOp::OrdinalRank
        | ListOp::TiedRank
        | ListOp::Union
        | ListOp::Intersect
        | ListOp::Remove => Val::N(f64::NAN),
        ListOp::Index | ListOp::Element => match (ctx.pick(op, l, n), ctx.e.tys[i as usize]) {
            (Some(slot), _) => slot_val(&slot),
            (None, Ty::Vec) => Val::V(f64::NAN, f64::NAN),
            (None, Ty::Bool) => Val::B(false, true),
            (None, _) => Val::N(f64::NAN),
        },
        _ => return None,
    })
}

/// The value of a [`Node::Call`] of this group that gives a number (see
/// `commands::call_value`).
pub(crate) fn call_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: CallOp,
    first: u32,
    len: u32,
) -> Option<Val> {
    Some(match op {
        CallOp::IndexOf => Val::N(ctx.index_of(first, len)),
        CallOp::Fold => ctx.fold(first, len, true),
        CallOp::SumFold => ctx.fold(first, len, false),
        // a list: built by `list`
        _ => {
            let _ = len;
            Val::N(f64::NAN)
        }
    })
}

/// `Kernel.MIN_PRECISION`, the slack of the bounds of `Sequence`.
const MIN_PRECISION: f64 = 1e-5;

/// `MyMath.LARGEST_INTEGER`.
const LARGEST_INTEGER: f64 = 9007199254740992.0;

/// The expression of a `Zip` at one element of each of its lists: the
/// variables are bound one inside the other (`updateLocalVar`).
fn zip_element<const M: bool>(ctx: &Ctx<'_, M>, expression: u32, bound: &[(u32, Slot)]) -> Slot {
    match bound.split_first() {
        Some(((depth, slot), rest)) => {
            let local = Local {
                depth: *depth,
                value: slot_val(slot),
                run: slot.run(),
                vector: slot.kind == Kind::Vector,
                text: slot.text_value(),
                up: ctx.locals,
            };
            zip_element(
                &Ctx {
                    locals: Some(&local),
                    ..*ctx
                },
                expression,
                rest,
            )
        }
        None => ctx.element(expression),
    }
}

impl Parser<'_> {
    /// Whether the subtree at `i` is a boolean.
    pub(crate) fn is_bool(&self, i: u32) -> bool {
        matches!(
            crate::expr::types::check(self.c, &self.nodes, i),
            Ok((tys, ..)) if tys[i as usize] == Ty::Bool
        )
    }

    /// Whether the subtree at `i` is a locus object.
    pub(crate) fn is_locus(&self, i: u32) -> bool {
        matches!(self.nodes[i as usize], Node::Obj(id) if self.c.kind(id) == Kind::Locus)
    }

    /// Whether every subtree in `args` is a number (`wrapInList` with
    /// `NUMERIC`).
    pub(crate) fn are_numbers(&self, args: &[u32]) -> bool {
        !args.is_empty() && args.iter().all(|&a| self.is_number(a))
    }

    /// Whether every subtree in `args` is a point or a vector
    /// (`wrapInList` with `POINT`).
    pub(crate) fn are_points(&self, args: &[u32]) -> bool {
        !args.is_empty() && args.iter().all(|&a| self.is_point_or_vector(a))
    }

    /// `Sequence(n)`, `Sequence(a, b)`, `Sequence(a, b, step)` and
    /// `Sequence(expression, k, from, to[, step])` (`CmdSequence`): with
    /// four or five arguments the second is the variable, which the
    /// expression reads and which hides an object of the same name.
    pub(crate) fn sequence(&mut self) -> Result<u32, String> {
        let variable = self.sequence_variable()?;
        let Some(variable) = variable else {
            return match self.arguments()?[..] {
                [n] => Ok(self.push(Node::Range(NONE, n, NONE))),
                [a, b] => Ok(self.push(Node::Range(a, b, NONE))),
                [a, b, s] => Ok(self.push(Node::Range(a, b, s))),
                _ => Err("`Sequence` takes one to five arguments".into()),
            };
        };
        self.expect(&Tok::LParen, "(")?;
        let depth = self.locals.len() as u32;
        self.locals.push((variable, Elem::Num));
        let expression = self.expression();
        self.locals.pop();
        let expression = expression?;
        self.expect(&Tok::Comma, ",")?;
        self.pos += 1;
        self.expect(&Tok::Comma, ",")?;
        let from = self.expression()?;
        self.expect(&Tok::Comma, ",")?;
        let to = self.expression()?;
        let step = if self.eat(&Tok::Comma) {
            self.expression()?
        } else {
            NONE
        };
        self.expect(&Tok::RParen, ")")?;
        Ok(self.push(Node::Seq(expression, from, to, step, depth)))
    }

    /// The variable of a `Sequence` with four or five arguments, found
    /// before its expression is parsed; `None` with fewer arguments.
    fn sequence_variable(&self) -> Result<Option<String>, String> {
        let commas = self.argument_commas();
        if commas.len() < 3 {
            return Ok(None);
        }
        if commas.len() > 4 {
            return Err("`Sequence` takes one to five arguments".into());
        }
        match &self.toks[commas[0] + 1].tok {
            Tok::Name { text, call: false } if commas[1] == commas[0] + 2 => Ok(Some(text.clone())),
            _ => Err("the second argument of `Sequence` must be a variable".into()),
        }
    }

    /// The arguments of a [`Node::Call`]: each one wrapped in an
    /// [`Node::Item`], so that they are one run of nodes whatever their
    /// own size; where it starts.
    pub(crate) fn call_run(&mut self, args: &[u32]) -> u32 {
        let first = self.nodes.len() as u32;
        for &a in args {
            self.push(Node::Item(a));
        }
        first
    }

    /// The variables of a `Zip` (`Zip(expression, k, l, m, n, ...)`), found
    /// before its expression is parsed; `None` when the arguments do not
    /// have its shape, an error when they almost do (`CmdZip`).
    fn zip_variables(&self) -> Result<Option<Vec<String>>, String> {
        let mut depth = 0usize;
        let mut commas = Vec::new();
        let mut end = None;
        for (j, t) in self.toks.iter().enumerate().skip(self.pos) {
            match t.tok {
                Tok::LParen | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j);
                        break;
                    }
                }
                Tok::Comma if depth == 1 => commas.push(j),
                _ => {}
            }
        }
        let end = end.ok_or("missing closing `)`")?;
        let n = commas.len() + 1;
        if n < 3 {
            return Ok(None);
        }
        if n % 2 == 0 {
            return Err("`Zip` takes an expression, then a variable and a list per pair".into());
        }
        let mut names = Vec::new();
        for k in 0..n / 2 {
            let var = commas[2 * k];
            let list = commas[2 * k + 1];
            let after = if 2 * k + 2 < commas.len() {
                commas[2 * k + 2]
            } else {
                end
            };
            if list != var + 2 || after <= list + 1 {
                return Err("every second argument of `Zip` must be a variable".into());
            }
            match &self.toks[var + 1].tok {
                Tok::Name { text, call: false } => names.push(text.clone()),
                _ => return Err("every second argument of `Zip` must be a variable".into()),
            }
        }
        Ok(Some(names))
    }

    /// `Zip(expression, k, list, ...)` (`CmdZip`): the expression is parsed
    /// with every variable of the call bound, each list after them without
    /// them, as GeoGebra resolves its arguments.
    pub(crate) fn zip(&mut self) -> Result<Option<u32>, String> {
        let Some(variables) = self.zip_variables()? else {
            return Ok(None);
        };
        let depth = self.locals.len() as u32;
        for name in &variables {
            // the lists are parsed after the expression, so the element
            // type is only known when they are; a list is what a variable
            // of a `Zip` reads as when its expression uses it as one
            self.locals.push((name.clone(), Elem::List));
        }
        self.expect(&Tok::LParen, "(")?;
        let expression = self.expression()?;
        self.locals.truncate(depth as usize);
        let mut lists = Vec::new();
        for _ in 0..variables.len() {
            self.expect(&Tok::Comma, ",")?;
            // the variable itself, already bound above
            self.pos += 1;
            self.expect(&Tok::Comma, ",")?;
            lists.push(self.expression()?);
        }
        self.expect(&Tok::RParen, ")")?;
        // one run of nodes: the expression, then a variable and its list
        let first = self.nodes.len() as u32;
        self.push(Node::Item(expression));
        for (k, list) in lists.iter().enumerate() {
            self.push(Node::Local(depth + k as u32, Elem::List));
            self.push(Node::Item(*list));
        }
        let len = self.nodes.len() as u32 - first;
        Ok(Some(self.push(Node::Call(CallOp::Zip, first, len))))
    }

    /// `Product(expression, k, from, to)` (`CmdSum.processSymb`), the
    /// shape of a four-argument `Sequence`: its variable is found before
    /// the expression is parsed; `None` for any other shape.
    pub(crate) fn fold(&mut self, text: &str) -> Result<Option<u32>, String> {
        let mut depth = 0usize;
        let mut commas = Vec::new();
        for (j, t) in self.toks.iter().enumerate().skip(self.pos) {
            match t.tok {
                Tok::LParen | Tok::LBrace => depth += 1,
                Tok::RParen | Tok::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Comma if depth == 1 => commas.push(j),
                _ => {}
            }
        }
        if commas.len() != 3 {
            return Ok(None);
        }
        let variable = match &self.toks[commas[0] + 1].tok {
            Tok::Name { text, call: false } if commas[1] == commas[0] + 2 => text.clone(),
            _ => return Ok(None),
        };
        // Over the function variable `x` the expression stays a function
        // of it: `resArgsLocalNumVar` binds the name, but `x^2` has
        // resolved to the function variable already, so every iteration
        // adds the same function (`Sum(x^2, x, 1, 3)` is the function
        // `3x^2`, not the number 14).
        let function_of_x = variable == "x";
        let local = self.locals.len() as u32;
        if !function_of_x {
            self.locals.push((variable, Elem::Num));
        }
        self.expect(&Tok::LParen, "(")?;
        let expression = self.expression()?;
        if !function_of_x {
            self.locals.pop();
        }
        self.expect(&Tok::Comma, ",")?;
        self.pos += 1;
        self.expect(&Tok::Comma, ",")?;
        let from = self.expression()?;
        self.expect(&Tok::Comma, ",")?;
        let to = self.expression()?;
        self.expect(&Tok::RParen, ")")?;
        if function_of_x {
            let count = self.push(Node::Bin(Bin::FoldCount, from, to));
            let node = if text == "Sum" {
                Node::Bin(Bin::Mul, expression, count)
            } else {
                Node::Bin(Bin::Pow, expression, count)
            };
            return Ok(Some(self.push(node)));
        }
        let first = self.nodes.len() as u32;
        self.push(Node::Item(expression));
        self.push(Node::Local(local, Elem::Num));
        self.push(Node::Item(from));
        self.push(Node::Item(to));
        let op = if text == "Sum" {
            CallOp::SumFold
        } else {
            CallOp::Fold
        };
        Ok(Some(self.push(Node::Call(op, first, 4))))
    }
}

impl<const M: bool> Ctx<'_, M> {
    /// `AlgoSequenceRange`: `Sequence(n)` is 1 to `(int) Math.round(n)` (a
    /// count past `int` wraps, one of 0 or less is empty); `Sequence(a, b)`
    /// runs from `a` to `b` rounded, up or down; `Sequence(a, b, step)` steps
    /// from `a`, on decimals when `a` and `step` have them and there are at
    /// most 100 steps.
    pub(crate) fn range(&self, from: u32, to: u32, step: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        if from == NONE {
            // `(int)` of the `long` keeps its low 32 bits
            let n = (math::java_round(self.full_number(to)) as i32).max(0) as u32;
            let start = lists.reserve(n)?;
            if !lists.charge(u64::from(n)) {
                return None;
            }
            for k in 0..n {
                lists.set(start + k, Slot::number(f64::from(k) + 1.0));
            }
            return Some((start, n));
        }
        let f = self.full(from);
        let s = (step != NONE).then(|| self.full(step));
        let (mut a, mut b) = (f.num(), self.full_number(to));
        if s.is_none() {
            a = math::java_round(a) as f64;
            b = math::java_round(b) as f64;
        }
        if a.abs() > LARGEST_INTEGER || b.abs() > LARGEST_INTEGER {
            return None;
        }
        let sign = if b >= a { 1.0 } else { -1.0 };
        let d = match &s {
            Some(s) => {
                let d = s.num();
                if math::is_zero(d) || d * sign < 0.0 {
                    return None;
                }
                d
            }
            None => sign,
        };
        let decimals = match (&f.dec, s.as_ref().and_then(|s| s.dec.as_ref())) {
            (Some(x), Some(y)) if (b - a) / d <= 100.0 => x.get(a).zip(y.get(d)),
            _ => None,
        };
        let decimals = decimals.as_ref().map(|(x, y)| (x.as_ref(), y.as_ref()));
        let values = |emit: &mut dyn FnMut(f64) -> bool| {
            if let Some((mut k, step)) = decimals.map(|(x, y)| (x.clone(), y)) {
                while k.to_f64() * sign <= b * sign + MIN_PRECISION {
                    if !emit(k.to_f64()) {
                        return;
                    }
                    match k.add(step) {
                        Some(next) => k = next,
                        None => return,
                    }
                }
                return;
            }
            let mut steps = 0.0;
            let mut k = a;
            while k * sign <= b * sign + MIN_PRECISION {
                if math::is_integer(k) {
                    k = math::java_round(k) as f64;
                }
                if !emit(k) {
                    return;
                }
                steps += 1.0;
                k = a + steps * d;
            }
        };
        let n = count(lists, values)?;
        let start = lists.reserve(n)?;
        if !lists.charge(u64::from(n)) {
            return None;
        }
        let mut k = 0;
        values(&mut |v| {
            lists.set(start + k, Slot::number(v));
            k += 1;
            k < n
        });
        Some((start, n))
    }

    /// `AlgoSequence`: the expression at the variable's values from `from`
    /// on by `step` while within `to` (and `MIN_PRECISION`); a value close
    /// to a whole number is made whole.
    pub(crate) fn sequence(
        &self,
        e: u32,
        from: u32,
        to: u32,
        step: u32,
        depth: u32,
    ) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (f, t) = (self.full_number(from), self.full_number(to));
        let s = if step == NONE {
            1.0
        } else {
            self.full_number(step)
        };
        if f.is_nan() || t.is_nan() || s.is_nan() {
            return None;
        }
        if (t - f) * s <= -MIN_PRECISION {
            return Some((lists.len(), 0));
        }
        if ((t - f) / s).is_infinite() {
            return None;
        }
        let values = |emit: &mut dyn FnMut(f64) -> bool| {
            let mut v = f;
            while (s > 0.0 && v <= t + MIN_PRECISION) || (s < 0.0 && v >= t - MIN_PRECISION) {
                if !emit(v) {
                    return;
                }
                v += s;
                if math::is_integer(v) {
                    v = math::java_round(v) as f64;
                }
            }
        };
        let n = count(lists, values)?;
        let start = lists.reserve(n)?;
        let mut k = 0;
        values(&mut |v| {
            work(1);
            if lists.over() {
                return false;
            }
            let local = Local {
                depth,
                value: Val::N(v),
                run: None,
                vector: false,
                text: None,
                up: self.locals,
            };
            let slot = Ctx {
                locals: Some(&local),
                ..*self
            }
            .element(e);
            lists.set(start + k, slot);
            k += 1;
            k < n
        });
        (!lists.over()).then_some((start, n))
    }

    /// `GeoElement.isEqual` of two list elements: numbers, booleans and
    /// points compare by value (points per coordinate, `isEqualPoint2D`),
    /// texts by their string (`GeoText.isEqualExtended`), undefined
    /// elements and lists never equal (two lists are distinct objects in
    /// GeoGebra).
    pub(super) fn same_element(&self, a: &Slot, b: &Slot) -> bool {
        match (a.kind, a.value, b.value) {
            (Kind::Number | Kind::Angle, Value::Number(x), Value::Number(y)) => near(x, y),
            (Kind::Boolean, Value::Bool(x), Value::Bool(y)) => x == y,
            (
                Kind::Point | Kind::Vector,
                Value::Point { x, y } | Value::Vector { x, y },
                Value::Point { x: u, y: v } | Value::Vector { x: u, y: v },
            ) => near(x, u) && near(y, v),
            (Kind::Text, _, _) => match (a.text_value(), b.text_value()) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            },
            // `GeoList.isEqualExtended`: two lists are equal when they have
            // as many elements and every pair of them is equal
            (Kind::List, Value::List { len: a_len }, Value::List { len: b_len }) => {
                a_len == b_len
                    && self.lists.is_some_and(|lists| {
                        (0..a_len).all(|k| {
                            self.same_element(
                                &lists.read(self.c, a.start + k),
                                &lists.read(self.c, b.start + k),
                            )
                        })
                    })
            }
            _ => false,
        }
    }

    /// `Product(list, frequencies)` (`AlgoStats1D` with `geoList2`): every
    /// value as often as its frequency says, a class list one longer than
    /// the frequencies through the midpoints of its bounds.
    pub(crate) fn product_frequencies(&self, l: u32, n: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        let Some((s, size)) = self.list(l) else {
            return f64::NAN;
        };
        let Some((fs, fsize)) = self.list(n) else {
            return f64::NAN;
        };
        let number = |i: u32| -> Option<f64> {
            let slot = lists.read(self.c, i);
            matches!(slot.kind, Kind::Number | Kind::Angle)
                .then(|| number_of(slot))
                .flatten()
        };
        // a list of frequencies shorter than the values throws in GeoGebra;
        // it is undefined here
        let midpoint = size == fsize + 1;
        let steps = if midpoint { size - 1 } else { size };
        if size == 0 || fsize < steps || !lists.charge(u64::from(size)) {
            return f64::NAN;
        }
        let mut product = 1.0;
        for k in 0..steps {
            let (val, freq) = match (number(s + k), number(fs + k)) {
                (Some(v), Some(f)) => (v, f),
                _ => return f64::NAN,
            };
            let val = if midpoint {
                match number(s + k + 1) {
                    Some(next) => (val + next) / 2.0,
                    None => return f64::NAN,
                }
            } else {
                val
            };
            if freq < 0.0 {
                return f64::NAN;
            }
            product *= math::my_pow(val, freq);
        }
        product
    }

    /// `IndexOf(object, list[, start])` (`AlgoIndexOf`): the first
    /// position of an equal element from `start` (truncated towards zero,
    /// from 1), undefined below 1 and when there is no hit.
    pub(crate) fn index_of(&self, first: u32, len: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        // `(int) start - 1`, which wraps like the int arithmetic of Java
        // (`IndexOf(1, l, -1e300)` starts at `int` max, one past the wrap)
        let start = if len == 3 {
            (self.full_number(first + 2) as i32).wrapping_sub(1)
        } else {
            0
        };
        if start < 0 {
            return f64::NAN;
        }
        let needle = self.element(first);
        let Some((s, size)) = self.list(first + 1) else {
            return f64::NAN;
        };
        if !lists.charge(u64::from(size)) {
            return f64::NAN;
        }
        for k in u32::try_from(start).unwrap_or(0)..size {
            if self.same_element(&lists.read(self.c, s + k), &needle) {
                return f64::from(k) + 1.0;
            }
        }
        f64::NAN
    }

    /// `Product(expression, k, from, to)` (`AlgoFoldExpression`): the
    /// expression at `from` (not truncated), then at every whole number up
    /// to `to`, multiplied. Points and vectors fold by coordinates
    /// (`PointNDFold`): added for a sum, multiplied as complex numbers for
    /// a product.
    pub(crate) fn fold(&self, first: u32, len: u32, multiply: bool) -> Val {
        let coordinates = self.e.tys[first as usize] == Ty::Vec;
        let undefined = if coordinates {
            Val::V(f64::NAN, f64::NAN)
        } else {
            Val::N(f64::NAN)
        };
        let Some(lists) = self.lists else {
            return undefined;
        };
        let (expression, variable) = (first, first + 1);
        let Node::Local(depth, _) = self.node(variable) else {
            return undefined;
        };
        let (a, b) = (self.full_number(first + 2), self.full_number(first + 3));
        if !a.is_finite() || !b.is_finite() || len != 4 {
            return undefined;
        }
        // one term: a number, or the coordinates of a point or vector
        let value = |v: f64| -> (f64, f64) {
            let local = Local {
                depth,
                value: Val::N(v),
                run: None,
                vector: false,
                text: None,
                up: self.locals,
            };
            let slot = Ctx {
                locals: Some(&local),
                ..*self
            }
            .element(expression);
            match (slot.kind, slot.value) {
                (Kind::Number | Kind::Angle, _) if !coordinates => {
                    (number_of(slot).unwrap_or(f64::NAN), 0.0)
                }
                (Kind::Point | Kind::Vector, Value::Point { x, y } | Value::Vector { x, y })
                    if coordinates =>
                {
                    (x, y)
                }
                _ => (f64::NAN, f64::NAN),
            }
        };
        // the first term, whatever the operation is
        let mut acc = value(a);
        // `(int) from + 1`, which wraps like the int arithmetic of Java,
        // and the loop compares the plain value of `to` (`i <= to.getDouble()`)
        let mut k = (a as i32).wrapping_add(1);
        while f64::from(k) <= b {
            if !lists.charge(1) {
                return undefined;
            }
            let term = value(f64::from(k));
            acc = match (coordinates, multiply) {
                (false, true) => (acc.0 * term.0, 0.0),
                (false, false) => (acc.0 + term.0, 0.0),
                (true, true) => (
                    acc.0 * term.0 - acc.1 * term.1,
                    acc.1 * term.0 + acc.0 * term.1,
                ),
                (true, false) => (acc.0 + term.0, acc.1 + term.1),
            };
            k = k.wrapping_add(1);
        }
        if coordinates {
            Val::V(acc.0, acc.1)
        } else {
            Val::N(acc.0)
        }
    }

    /// The element `l(n)` or `Element(l, n)` picks, `None` when there is
    /// none. Its list is built and stays in the scratch buffer.
    pub(crate) fn pick(&self, op: ListOp, l: u32, n: u32) -> Option<Slot> {
        let lists = self.lists?;
        let mark = lists.len();
        let index = self.full_number(n);
        lists.truncate(mark);
        let (start, len) = self.list(l)?;
        let mut k = math::java_round(index).saturating_sub(1);
        if op == ListOp::Index && k < 0 {
            k += i64::from(len) + 1;
        }
        if (0..i64::from(len)).contains(&k) {
            return Some(lists.read(self.c, start + k as u32));
        }
        if op != ListOp::Index || len == 0 {
            return None;
        }
        // A missing row of a numeric matrix is a row of undefined numbers.
        // The shape is read again on each evaluation because the matrix can change.
        let (_, width) = lists.read(self.c, start).run()?;
        if width == 0 || !lists.charge(u64::from(len) * (u64::from(width) + 1)) {
            return None;
        }
        for i in 0..len {
            let (row, cols) = lists.read(self.c, start + i).run()?;
            if cols != width || (0..cols).any(|j| lists.read(self.c, row + j).kind != Kind::Number)
            {
                return None;
            }
        }
        // New scratch slots are initialized to undefined numbers.
        Some(Slot::list(lists.reserve(width)?, width))
    }

    /// A list command whose result is a list, built at the end of the
    /// scratch buffer.
    pub(crate) fn list_fn(&self, op: ListOp, l: u32, n: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        if let Some(run) = self.locus_run(op, l, n) {
            return run;
        }
        match op {
            ListOp::Append | ListOp::Prepend => {
                let (s, len) = self.list(l)?;
                let start = lists.reserve(len + 1)?;
                if !lists.charge(u64::from(len) + 1) {
                    return None;
                }
                let at = if op == ListOp::Append {
                    start
                } else {
                    start + 1
                };
                for k in 0..len {
                    lists.set(at + k, lists.read(self.c, s + k));
                }
                let object = self.element(n);
                let at = if op == ListOp::Append {
                    start + len
                } else {
                    start
                };
                lists.set(at, object);
                Some((start, len + 1))
            }
            ListOp::First | ListOp::Last => {
                // `(int) n`, 1 without `n`; undefined for an empty list
                let mark = lists.len();
                let count = if n == NONE { 1.0 } else { self.full_number(n) };
                lists.truncate(mark);
                let (s, len) = self.list(l)?;
                let count = count as i32;
                let count = u32::try_from(count).ok().filter(|&c| len > 0 && c <= len)?;
                let from = if op == ListOp::First {
                    s
                } else {
                    s + len - count
                };
                if matches!(
                    self.e.tys[l as usize],
                    Ty::List(Elem::Vec | Elem::Object | Elem::Any)
                ) {
                    // AlgoFirst and AlgoLast copy graph points and segments.
                    let start = lists.reserve(count)?;
                    if !lists.charge(u64::from(count)) {
                        return None;
                    }
                    for k in 0..count {
                        lists.set(
                            start + k,
                            lists.read(self.c, from + k).detach_graph_object(),
                        );
                    }
                    return Some((start, count));
                }
                if s >> OBJECT_SHIFT != 0 {
                    // a part of a list object read in place: it never
                    // changes, so it is read where it is
                    return Some((from, count));
                }
                self.copy_run(from, count)
            }
            ListOp::Reverse => {
                let (s, len) = self.list(l)?;
                let start = lists.reserve(len)?;
                if !lists.charge(u64::from(len)) {
                    return None;
                }
                for k in 0..len {
                    lists.set(start + k, lists.read(self.c, s + len - 1 - k));
                }
                Some((start, len))
            }
            ListOp::Sort if n == NONE => {
                let (s, len) = self.list(l)?;
                let first = (len > 0).then(|| lists.read(self.c, s))?;
                if !lists.charge(u64::from(len) * u64::from(1 + len.ilog2())) {
                    return None;
                }
                // one class for every element (`AlgoSort`: `geo.getClass()`),
                // of a kind its comparator knows: a number, an angle or a
                // boolean (all `NumberValue`, `GeoNumeric.getComparator`),
                // a point (`GeoPoint.getComparatorX`) or a text
                // (`GeoText.getComparator`)
                if !matches!(
                    first.kind,
                    Kind::Number | Kind::Angle | Kind::Boolean | Kind::Point | Kind::Text
                ) || (0..len).any(|k| lists.read(self.c, s + k).kind != first.kind)
                {
                    return None;
                }
                let (start, len) = self.copy_run(s, len)?;
                if first.kind == Kind::Text {
                    lists.sort_texts(start, len);
                } else {
                    lists.sort(start, len);
                }
                Some((start, len))
            }
            ListOp::Join => {
                let (s, len) = self.list(l)?;
                let mut total = 0u32;
                if !lists.charge(u64::from(len)) {
                    return None;
                }
                for k in 0..len {
                    let slot = lists.read(self.c, s + k);
                    match (slot.kind, slot.run()) {
                        (_, Some((_, m))) => total = total.checked_add(m)?,
                        // an undefined list has no elements
                        (Kind::List, None) => {}
                        _ => return None,
                    }
                }
                let start = lists.reserve(total)?;
                if !lists.charge(u64::from(total)) {
                    return None;
                }
                let mut at = start;
                for k in 0..len {
                    if let Some((from, m)) = lists.read(self.c, s + k).run() {
                        for j in 0..m {
                            lists.set(at + j, lists.read(self.c, from + j));
                        }
                        at += m;
                    }
                }
                Some((start, total))
            }
            ListOp::Sort if n != NONE => {
                // `AlgoSort(cons, label, valueList, inputList)`: the first
                // list holds the values, the second the keys that order them
                let (vs, vlen) = self.list(l)?;
                let (s, len) = self.list(n)?;
                // an empty or differently sized value list is undefined
                if len == 0 || vlen != len {
                    return None;
                }
                // every key of the class of the first one (`geo.getClass()`),
                // of a kind its comparator knows: a number (an angle is a
                // class of its own, `GeoAngle` next to `GeoNumeric`), a
                // point or a text; a vector is neither a point nor a number
                let class = |slot: &Slot| -> Option<u8> {
                    match (slot.kind, slot.value) {
                        (Kind::Number, Value::Number(_)) => Some(0),
                        (Kind::Angle, Value::Number(_)) => Some(1),
                        (Kind::Point, Value::Point { .. }) => Some(2),
                        (Kind::Text, _) => Some(3),
                        _ => None,
                    }
                };
                let first = class(&lists.read(self.c, s))?;
                if (0..len).any(|k| class(&lists.read(self.c, s + k)) != Some(first)) {
                    return None;
                }
                let mut order: Vec<u32> = (0..len).collect();
                let number_key = |k: u32| {
                    let slot = lists.read(self.c, s + k);
                    match slot.value {
                        Value::Number(v) => (v + 0.0, 0.0),
                        Value::Point { x, y } => (x + 0.0, y + 0.0),
                        _ => (f64::NAN, f64::NAN),
                    }
                };
                let text_key = |k: u32| {
                    let slot = lists.read(self.c, s + k);
                    let text = slot.text_value().unwrap_or("").to_owned();
                    (
                        crate::expr::commands::text::strings::remove_accents(&text),
                        text,
                    )
                };
                if !lists.charge(u64::from(len) * u64::from(1 + len.ilog2())) {
                    return None;
                }
                // `GeoNumeric.getComparator` never returns 0 for two keys,
                // so the `TreeSet` keeps both of them; the construction
                // indexes of the elements of one list are all equal, so the
                // comparator answers `+1` for two of them and the later one
                // lands to the right: equal keys keep the order of the
                // list, and `GeoList.find` (`indexOf`) returns the position
                // of each key itself: a stable sort of the values
                if first == 3 {
                    order.sort_by(|&a, &b| {
                        let (ka, kb) = (text_key(a), text_key(b));
                        crate::expr::commands::text::strings::java_cmp(&ka.0, &kb.0)
                            .then_with(|| {
                                crate::expr::commands::text::strings::java_cmp(&ka.1, &kb.1)
                            })
                            .then(a.cmp(&b))
                    });
                } else {
                    order.sort_by(|&a, &b| {
                        let (ka, kb) = (number_key(a), number_key(b));
                        ka.0.is_nan()
                            .cmp(&kb.0.is_nan())
                            .then(ka.0.total_cmp(&kb.0))
                            .then(ka.1.total_cmp(&kb.1))
                            .then(a.cmp(&b))
                    });
                }
                let start = lists.reserve(len)?;
                for (at, &k) in order.iter().enumerate() {
                    lists.set(start + at as u32, lists.read(self.c, vs + k));
                }
                Some((start, len))
            }
            ListOp::RemoveUndefined => {
                let (s, len) = self.list(l)?;
                if !lists.charge(u64::from(len)) {
                    return None;
                }
                let start = lists.reserve(len)?;
                let mut at = start;
                for k in 0..len {
                    let slot = lists.read(self.c, s + k);
                    if !matches!(slot.value, Value::Undefined) {
                        lists.set(at, slot);
                        at += 1;
                    }
                }
                Some((start, at - start))
            }
            ListOp::Unique => self.unique(l),
            ListOp::Flatten => {
                let (s, len) = self.list(l)?;
                let mut leaves: Vec<Slot> = Vec::new();
                if !self.flatten_into(s, len, &mut leaves) {
                    return None;
                }
                let start = lists.reserve(leaves.len() as u32)?;
                for (k, slot) in leaves.iter().enumerate() {
                    lists.set(start + k as u32, slot.clone());
                }
                Some((start, leaves.len() as u32))
            }
            ListOp::PointList => {
                let (s, len) = self.list(l)?;
                let mut points: Vec<Slot> = Vec::new();
                // `AlgoPointList`: an empty input gives an undefined list
                if len == 0 {
                    return None;
                }
                if !lists.charge(u64::from(len)) {
                    return None;
                }
                for k in 0..len {
                    let Some((rs, rn)) = lists.read(self.c, s + k).run() else {
                        continue;
                    };
                    if rn != 2 {
                        continue;
                    }
                    let number = |i: u32| -> Option<f64> {
                        let slot = lists.read(self.c, i);
                        matches!(slot.kind, Kind::Number | Kind::Angle)
                            .then(|| number_of(slot))
                            .flatten()
                    };
                    if let (Some(x), Some(y)) = (number(rs), number(rs + 1)) {
                        if !lists.charge(1) {
                            return None;
                        }
                        points.push(Slot::coords(Kind::Point, x, y));
                    }
                }
                let start = lists.reserve(points.len() as u32)?;
                for (k, slot) in points.iter().enumerate() {
                    lists.set(start + k as u32, slot.clone());
                }
                Some((start, points.len() as u32))
            }
            ListOp::OrdinalRank | ListOp::TiedRank => {
                let (s, len) = self.list(l)?;
                let tied = op == ListOp::TiedRank;
                if len == 0 {
                    return None;
                }
                if !lists.charge(u64::from(len).saturating_mul(u64::from(1 + len.ilog2()))) {
                    return None;
                }
                let mut values: Vec<(f64, u32)> = Vec::with_capacity(len as usize);
                for k in 0..len {
                    let slot = lists.read(self.c, s + k);
                    let v = matches!(slot.kind, Kind::Number | Kind::Angle)
                        .then(|| number_of(slot))
                        .flatten()?;
                    if v.is_nan() {
                        return None;
                    }
                    values.push((v, k));
                }
                values.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                let mut ranks = vec![f64::NAN; len as usize];
                let mut k = 0;
                while k < values.len() {
                    let mut end = k + 1;
                    while end < values.len() && values[end].0.total_cmp(&values[k].0).is_eq() {
                        end += 1;
                    }
                    for (rank, (_, index)) in values[k..end].iter().enumerate() {
                        ranks[*index as usize] = if tied {
                            // `TiesStrategy.AVERAGE`
                            (f64::from(k as u32) + f64::from(end as u32) + 1.0) / 2.0
                        } else {
                            // `AlgoOrdinalRank`: positions in order, ties by
                            // their index (the comparator never returns 0)
                            rank as f64 + 1.0 + k as f64
                        };
                    }
                    k = end;
                }
                let start = lists.reserve(len)?;
                for (k, rank) in ranks.iter().enumerate() {
                    lists.set(start + k as u32, Slot::number(*rank));
                }
                Some((start, len))
            }
            ListOp::Union | ListOp::Intersect | ListOp::Remove => {
                let (s, len) = self.list(l)?;
                let (ts, tlen) = self.list(n)?;
                if !lists.charge(u64::from(len) + u64::from(tlen)) {
                    return None;
                }
                let read = |i: u32| lists.read(self.c, i);
                match op {
                    ListOp::Union => {
                        let mut out: Vec<Slot> = Vec::new();
                        for k in 0..len + tlen {
                            let slot = read(if k < len { s + k } else { ts + k - len });
                            if !lists.charge(u64::from(out.len() as u32) + 1) {
                                return None;
                            }
                            if !out.iter().any(|kept| self.same_element(kept, &slot)) {
                                out.push(slot);
                            }
                        }
                        let start = lists.reserve(out.len() as u32)?;
                        for (k, slot) in out.iter().enumerate() {
                            lists.set(start + k as u32, slot.clone());
                        }
                        Some((start, out.len() as u32))
                    }
                    ListOp::Intersect => {
                        // the second list gives the order (`AlgoIntersection`)
                        let mut out: Vec<Slot> = Vec::new();
                        for k in 0..tlen {
                            let slot = read(ts + k);
                            if !lists.charge(u64::from(len) + 1) {
                                return None;
                            }
                            let shared = (0..len).any(|j| self.same_element(&read(s + j), &slot));
                            if shared && !out.iter().any(|kept| self.same_element(kept, &slot)) {
                                out.push(slot);
                            }
                        }
                        let start = lists.reserve(out.len() as u32)?;
                        for (k, slot) in out.iter().enumerate() {
                            lists.set(start + k as u32, slot.clone());
                        }
                        Some((start, out.len() as u32))
                    }
                    _ => {
                        // `AlgoRemove`: the defined elements of the second
                        // list, and as many undefined ones as it has
                        let mut remove: Vec<Slot> = Vec::new();
                        let mut undefined = 0u32;
                        for k in 0..tlen {
                            let slot = read(ts + k);
                            if matches!(slot.value, Value::Undefined) {
                                undefined += 1;
                            } else {
                                remove.push(slot);
                            }
                        }
                        // every element scans the ones still to remove
                        // (`AlgoRemove`), and a removal shifts the rest of
                        // them, which is the work of a product
                        let scans = u64::from(len).saturating_mul(u64::from(tlen));
                        if !lists.charge(scans) {
                            return None;
                        }
                        let mut out: Vec<Slot> = Vec::new();
                        for k in 0..len {
                            let slot = read(s + k);
                            let mut drop = false;
                            if undefined > 0 && matches!(slot.value, Value::Undefined) {
                                undefined -= 1;
                                drop = true;
                            }
                            if !drop {
                                if let Some(j) = (0..remove.len())
                                    .find(|&j| self.same_element(&remove[j], &slot))
                                {
                                    remove.remove(j);
                                    drop = true;
                                }
                            }
                            if !drop {
                                out.push(slot);
                            }
                        }
                        let start = lists.reserve(out.len() as u32)?;
                        for (k, slot) in out.iter().enumerate() {
                            lists.set(start + k as u32, slot.clone());
                        }
                        Some((start, out.len() as u32))
                    }
                }
            }
            _ => None,
        }
    }

    /// `First(locus, n)` (`AlgoFirstLocus`): the first points of the
    /// locus; the outer `None` is any other command or source, the inner
    /// one an undefined result.
    fn locus_run(&self, op: ListOp, l: u32, n: u32) -> Option<Option<(u32, u32)>> {
        if op != ListOp::First {
            return None;
        }
        let Node::Obj(id) = self.node(l) else {
            return None;
        };
        if self.c.kind(id) != Kind::Locus {
            return None;
        }
        let run = (|| -> Option<(u32, u32)> {
            let lists = self.lists?;
            let points = &self.c.locus(id)?.points;
            let outsize = if n == NONE { 1.0 } else { self.full_number(n) } as i32;
            if points.is_empty() || outsize < 0 || outsize > points.len() as i32 {
                return None;
            }
            if outsize == 0 {
                return Some((lists.len(), 0));
            }
            let start = lists.reserve(outsize as u32)?;
            if !lists.charge(u64::from(outsize as u32)) {
                return None;
            }
            for k in 0..outsize {
                let p = &points[k as usize];
                lists.set(start + k as u32, Slot::coords(Kind::Point, p.x, p.y));
            }
            Some((start, outsize as u32))
        })();
        Some(run)
    }

    /// `Unique(list)` (`AlgoUnique`): the numbers of `FrequencyGgb` — its
    /// `TreeMap` with the rounding comparator, so one bucket per value and
    /// the buckets in order — every value within `STANDARD_PRECISION` of an
    /// earlier one left out; texts come out as the strings of `new
    /// Frequency()`, deduplicated and in natural order; other kinds keep
    /// their first appearance (`AlgoUnion.addToOutputList`).
    fn unique(&self, l: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (s, len) = self.list(l)?;
        if len == 0 {
            // `dataList.size() == 0`: undefined
            return None;
        }
        // one ordered lookup per element, like the sort of the output
        if !lists.charge(u64::from(len).saturating_mul(u64::from(1 + len.ilog2()))) {
            return None;
        }
        let numbers =
            (0..len).all(|k| matches!(lists.read(self.c, s + k).kind, Kind::Number | Kind::Angle));
        let texts = (0..len).all(|k| lists.read(self.c, s + k).kind == Kind::Text);
        let out: Vec<Slot> = if numbers {
            // `FrequencyGgb.addValue` first pulls a value close to a
            // five-digit decimal fraction onto it, and a value within
            // `STANDARD_PRECISION` of a kept one joins its bucket; the
            // buckets of one table are never near each other, so the
            // candidates in the window are few
            let mut kept: BTreeSet<Known> = BTreeSet::new();
            let near_kept = |kept: &BTreeSet<Known>, v: f64| {
                // the window is widened, for the subtraction of its bounds
                // rounds; the kept buckets are never near each other, so
                // the candidates in it are few
                kept.range(
                    Known(v - 3.0 * STANDARD_PRECISION)..=Known(v + 3.0 * STANDARD_PRECISION),
                )
                .any(|&Known(k)| near(k, v))
            };
            let mut out = Vec::new();
            let mut undefined = 0u32;
            for k in 0..len {
                let v = Self::check_decimal_fraction(number_of(lists.read(self.c, s + k))?);
                if v.is_nan() {
                    // no two undefined values are equal (`DoubleUtil.isEqual`)
                    undefined += 1;
                    continue;
                }
                if !near_kept(&kept, v) {
                    kept.insert(Known(v));
                    out.push(Slot::number(v));
                }
            }
            out.sort_unstable_by(|a, b| {
                let key = |s: &Slot| match s.value {
                    Value::Number(v) => v,
                    _ => f64::NAN,
                };
                key(a).total_cmp(&key(b))
            });
            out.extend(std::iter::repeat_n(
                Slot::undefined(Kind::Number),
                undefined as usize,
            ));
            out
        } else if texts {
            let mut strings: BTreeSet<Vec<u16>> = BTreeSet::new();
            for k in 0..len {
                strings.insert(
                    lists
                        .read(self.c, s + k)
                        .text_value()
                        .unwrap_or("")
                        .encode_utf16()
                        .collect(),
                );
            }
            strings
                .iter()
                .map(|units| {
                    Slot::text(
                        crate::expr::commands::text::strings::from_utf16(units),
                        None,
                    )
                })
                .collect()
        } else {
            // the first appearance is found by a scan, whose whole work is
            // charged like any other (`addToOutputList`)
            if !lists.charge(u64::from(len).saturating_mul(u64::from(len))) {
                return None;
            }
            let mut out: Vec<Slot> = Vec::new();
            for k in 0..len {
                let slot = lists.read(self.c, s + k);
                if !out.iter().any(|kept| self.same_element(kept, &slot)) {
                    out.push(slot);
                }
            }
            out
        };
        let start = lists.reserve(out.len() as u32)?;
        for (k, slot) in out.iter().enumerate() {
            lists.set(start + k as u32, slot.clone());
        }
        Some((start, out.len() as u32))
    }

    /// The leaves of the run `s..s + len` into `out` (`AlgoFlatten`);
    /// `false` past the element bound.
    fn flatten_into(&self, s: u32, len: u32, out: &mut Vec<Slot>) -> bool {
        let Some(lists) = self.lists else {
            return false;
        };
        for k in 0..len {
            let slot = lists.read(self.c, s + k);
            match slot.run() {
                Some((rs, rn)) => {
                    if !self.flatten_into(rs, rn, out) {
                        return false;
                    }
                }
                None => {
                    if !lists.charge(1) {
                        return false;
                    }
                    out.push(slot);
                }
            }
        }
        true
    }

    /// `DoubleUtil.checkDecimalFraction`: a value close (within
    /// `STANDARD_PRECISION` after scaling by `1e5`) to a decimal fraction
    /// of five digits, pulled onto it.
    fn check_decimal_fraction(val: f64) -> f64 {
        let frac = val * 1e5;
        let round = frac.round();
        if (frac - round).abs() <= STANDARD_PRECISION {
            round / 1e5
        } else {
            val
        }
    }

    /// `Classes(list, n | start, width)` (`AlgoClasses`).
    fn classes(&self, first: u32, len: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (s, size) = self.list(first)?;
        if size == 0 {
            return None;
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for k in 0..size {
            let slot = lists.read(self.c, s + k);
            let v = matches!(slot.kind, Kind::Number | Kind::Angle)
                .then(|| number_of(slot))
                .flatten()?;
            min = min.min(v);
            max = max.max(v);
        }
        if !lists.charge(u64::from(size)) {
            return None;
        }
        let bounds: Vec<f64> = if len == 2 {
            // a number of classes; one below (after `(int)`) is undefined
            let n = self.full_number(first + 1) as i32;
            if n < 1 {
                return None;
            }
            // the bounds are charged before they are built: a huge number
            // of classes must not allocate first
            if !lists.charge(u64::from(n as u32) + 1) {
                return None;
            }
            let width = (max - min) / f64::from(n);
            let mut out: Vec<f64> = (0..n).map(|k| min + f64::from(k) * width).collect();
            out.push(max);
            out
        } else {
            // a start and a width, up to the largest value
            let width = self.full_number(first + 2);
            if width <= 0.0 {
                return None;
            }
            let mut out = vec![self.full_number(first + 1)];
            while *out.last()? < max {
                if !lists.charge(1) {
                    return None;
                }
                out.push(out.last()? + width);
            }
            // one bound alone is undefined
            if out.len() < 2 {
                return None;
            }
            out
        };
        let start = lists.reserve(bounds.len() as u32)?;
        if len != 2 && !lists.charge(u64::from(bounds.len() as u32)) {
            // the bounds of a number of classes are charged before they
            // are built
            return None;
        }
        for (k, v) in bounds.iter().enumerate() {
            lists.set(start + k as u32, Slot::number(*v));
        }
        Some((start, bounds.len() as u32))
    }

    /// `Frequency` in its forms (`AlgoFrequency`).
    fn frequency(&self, first: u32, len: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let arg = |k: u32| first + k;
        let boolean = |k: u32| match self.full(arg(k)).val {
            Val::B(b, true) => b,
            _ => false,
        };
        // (isCumulative, classList, dataList, useDensity, scale factor)
        let (cumulative, class_list, data_list, density, factor) = match len {
            1 => (false, NONE, arg(0), false, NONE),
            2 if matches!(self.e.tys[arg(0) as usize], Ty::Bool) => {
                (boolean(0), NONE, arg(1), false, NONE)
            }
            2 => (false, arg(0), arg(1), false, NONE),
            3 if matches!(self.e.tys[arg(0) as usize], Ty::Bool) => {
                (boolean(0), arg(1), arg(2), false, NONE)
            }
            3 => (false, arg(0), arg(1), boolean(2), NONE),
            4 if matches!(self.e.tys[arg(0) as usize], Ty::Bool) => {
                (boolean(0), arg(1), arg(2), boolean(3), NONE)
            }
            4 => (false, arg(0), arg(1), boolean(2), arg(3)),
            _ => (boolean(0), arg(1), arg(2), boolean(3), arg(4)),
        };
        // `density`: a factor of zero or less is undefined
        let factor = if factor == NONE {
            1.0
        } else {
            let v = self.full_number(factor);
            if v <= 0.0 {
                return None;
            }
            v
        };
        let (ds, dsize) = self.list(data_list)?;
        if dsize == 0 {
            return None;
        }
        // the table of `Frequency`: one ordered bucket per value, so both
        // building it and reading a count cost a lookup, not a scan
        if !lists.charge(u64::from(dsize).saturating_mul(u64::from(1 + dsize.ilog2()))) {
            return None;
        }
        let texts = (0..dsize).all(|k| lists.read(self.c, ds + k).kind == Kind::Text);
        let table = if texts {
            // `new Frequency()`: exact strings, in natural order
            let mut table: BTreeMap<Vec<u16>, u64> = BTreeMap::new();
            for k in 0..dsize {
                let slot = lists.read(self.c, ds + k);
                let text = slot.text_value().unwrap_or("").to_owned();
                *table.entry(text.encode_utf16().collect()).or_insert(0) += 1;
            }
            Table::Texts(table)
        } else {
            // `FrequencyGgb.addValue` first pulls a value close to a
            // five-digit decimal fraction onto it, and a value within
            // `STANDARD_PRECISION` of a kept one joins its bucket; the
            // buckets of one table are never near each other, so the
            // candidates in the window are few
            let mut table: BTreeMap<Known, u64> = BTreeMap::new();
            let mut undefined = 0u32;
            for k in 0..dsize {
                let v = Self::check_decimal_fraction(
                    matches!(lists.read(self.c, ds + k).kind, Kind::Number | Kind::Angle)
                        .then(|| number_of(lists.read(self.c, ds + k)))
                        .flatten()?,
                );
                if v.is_nan() {
                    // no two undefined values are equal, so each is its own
                    // bucket (`DoubleUtil.isEqual`)
                    undefined += 1;
                    continue;
                }
                let near = table
                    .range(
                        Known(v - 3.0 * STANDARD_PRECISION)..=Known(v + 3.0 * STANDARD_PRECISION),
                    )
                    .find(|&(kept, _)| near(kept.0, v))
                    .map(|(kept, _)| *kept);
                match near {
                    Some(kept) => *table.get_mut(&kept).unwrap() += 1,
                    None => {
                        table.insert(Known(v), 1);
                    }
                }
            }
            Table::Numbers(table, undefined)
        };
        let mut out: Vec<f64> = Vec::new();
        if class_list == NONE {
            // one count per value, in the order of `Frequency.valuesIterator`
            let mut write = |value: f64| -> bool {
                if !lists.charge(1) {
                    return false;
                }
                out.push(value);
                true
            };
            match &table {
                Table::Texts(table) => {
                    let mut cumulative_count = 0u64;
                    for count in table.values() {
                        cumulative_count += count;
                        let c = if cumulative { cumulative_count } else { *count };
                        if !write(c as f64) {
                            return None;
                        }
                    }
                }
                Table::Numbers(table, undefined) => {
                    // a cumulative count reads the running sums, a lookup
                    // per value like the one of a plain count
                    let running = cumulative.then(|| Cumulative::new(table));
                    if running.is_some()
                        && !lists.charge(u64::from(dsize) * u64::from(1 + dsize.ilog2()))
                    {
                        return None;
                    }
                    for (v, count) in table {
                        let c = match &running {
                            Some(running) => running.up_to(v.0),
                            None => *count,
                        };
                        if !write(c as f64) {
                            return None;
                        }
                    }
                    for _ in 0..*undefined {
                        // an undefined value counts one, and nothing is
                        // below it
                        if !write(if cumulative { 0.0 } else { 1.0 }) {
                            return None;
                        }
                    }
                }
            }
        } else {
            let (cs, csize) = self.list(class_list)?;
            if csize < 2 {
                return None;
            }
            let mut bounds = Vec::with_capacity(csize as usize);
            for k in 0..csize {
                let slot = lists.read(self.c, cs + k);
                let v = matches!(slot.kind, Kind::Number | Kind::Angle)
                    .then(|| number_of(slot))
                    .flatten()?;
                bounds.push(Self::check_decimal_fraction(v));
            }
            // every class reads its bounds off the running sums of the
            // table (a lookup each) and its two counts
            if !lists.charge(u64::from(csize) * u64::from(1 + dsize.ilog2())) {
                return None;
            }
            // the classes of a text table would compare a string with a
            // number, which GeoGebra refuses with an error
            let Table::Numbers(table, _) = &table else {
                return None;
            };
            let running = Cumulative::new(table);
            let mut cumulative_sum = 0.0;
            for i in 1..bounds.len() {
                let (lower, upper, increasing) = if bounds[i - 1] > bounds[i] {
                    (bounds[i], bounds[i - 1], false)
                } else {
                    (bounds[i - 1], bounds[i], true)
                };
                let mut freq = running.up_to(upper) as f64 - running.up_to(lower) as f64
                    + table_count(table, lower) as f64;
                // the highest class holds both its bounds
                if (i != bounds.len() - 1 && increasing) || (i != 1 && !increasing) {
                    freq -= table_count(table, upper) as f64;
                }
                if cumulative {
                    cumulative_sum += freq;
                }
                let v = if cumulative { cumulative_sum } else { freq };
                let v = if density {
                    factor * v / (upper - lower)
                } else {
                    v
                };
                if !lists.charge(1) {
                    return None;
                }
                out.push(v);
            }
        }
        let start = lists.reserve(out.len() as u32)?;
        for (k, v) in out.iter().enumerate() {
            lists.set(start + k as u32, Slot::number(*v));
        }
        Some((start, out.len() as u32))
    }

    /// `Zip(expression, k, list, ...)` (`AlgoZip`): the expression at the
    /// elements the lists share, as many as the shortest list holds.
    fn zip(&self, first: u32, len: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let expression = first;
        let pairs = (len - 1) / 2;
        let mut bound: Vec<(u32, u32, u32)> = Vec::with_capacity(pairs as usize);
        for k in 0..pairs {
            let local = first + 1 + 2 * k;
            let list = match self.node(first + 2 + 2 * k) {
                Node::Item(a) => a,
                _ => return None,
            };
            let Node::Local(depth, _) = self.node(local) else {
                return None;
            };
            // an undefined list makes the whole result undefined
            let (s, n) = self.list(list)?;
            bound.push((depth, s, n));
        }
        let n = bound.iter().map(|&(_, _, n)| n).min().unwrap_or(0);
        let start = lists.reserve(n)?;
        if !lists.charge(u64::from(n)) {
            return None;
        }
        for i in 0..n {
            work(1);
            if lists.over() {
                return None;
            }
            let slots: Vec<(u32, Slot)> = bound
                .iter()
                .map(|&(depth, s, _)| (depth, lists.read(self.c, s + i)))
                .collect();
            let slot = zip_element(self, expression, &slots);
            lists.set(start + i, slot);
        }
        Some((start, n))
    }

    /// A command behind a [`Node::Call`] whose result is a list (see
    /// `commands::call_list`): its `len` arguments from `first` on.
    pub(crate) fn call_list(&self, op: CallOp, first: u32, len: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        match op {
            CallOp::Take => {
                let (m, end) = (self.full_number(first + 1), self.full_number(first + 2));
                let to = len == 3;
                // an undefined bound gives an empty list (`AlgoTake`)
                if m.is_nan() || (to && end.is_nan()) {
                    return Some((lists.len(), 0));
                }
                let (s, size) = self.list(first)?;
                let (start, end) = (m as i32, if to { end as i32 } else { size as i32 });
                if end == 0 && size > 0 && start > 0 && start <= size as i32 {
                    return Some((lists.len(), 0));
                }
                if size == 0 || start <= 0 || end > size as i32 || start > end {
                    return None;
                }
                self.copy_run(s + start as u32 - 1, (end - start + 1) as u32)
            }
            CallOp::Insert => {
                let object = self.element(first);
                let (s, size) = self.list(first + 1)?;
                let mut at = self.full_number(first + 2) as i32;
                // -1 is after the last element, -2 before it
                if at < 0 {
                    at = size as i32 + at + 2;
                }
                if matches!(object.value, Value::Undefined) || at <= 0 || at > size as i32 + 1 {
                    return None;
                }
                let run = object.run();
                let total = size + run.map_or(1, |(_, n)| n);
                if !lists.charge(u64::from(total)) {
                    return None;
                }
                let start = lists.reserve(total)?;
                let insert_at = at as u32;
                let write = |at: &mut u32| match run {
                    // a list inserts its elements (`AlgoInsert`)
                    Some((rs, rn)) => {
                        for j in 0..rn {
                            lists.set(*at, lists.read(self.c, rs + j));
                            *at += 1;
                        }
                    }
                    None => {
                        lists.set(*at, object.clone());
                        *at += 1;
                    }
                };
                let mut at = start;
                for k in 0..size {
                    if k + 1 == insert_at {
                        write(&mut at);
                    }
                    lists.set(at, lists.read(self.c, s + k));
                    at += 1;
                }
                if insert_at == size + 1 {
                    write(&mut at);
                }
                Some((start, at - start))
            }
            CallOp::Classes => self.classes(first, len),
            CallOp::Frequency => self.frequency(first, len),
            CallOp::Zip => self.zip(first, len),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
