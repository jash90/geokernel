// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/MyList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdCAStoOperation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIdentity.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoTranspose.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoDimension.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOneListFunction.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Geometry and vector commands computed inside expressions.

#[path = "../../algo/conic_coefficients.rs"]
mod conic_coefficients;
#[path = "../../algo/conic_interval.rs"]
mod conic_interval;
pub(super) mod function_roots;

use crate::algo::intersect::Hint;
use crate::algo::vector::{matrix, VectorDef};
use crate::expr::eval::{combine, number_of, slot_val, Ctx, Val};
use crate::expr::list::Slot;
use crate::expr::parse::{list_form, Parser};
use crate::expr::{self, Bin, Elem, Entry, IfMode, ListOp, Node, Raw, Ty, Un, NONE};
use crate::kernel::{Construction, Def, Id, Kind, Value};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &[
    "Length",
    "Cross",
    "Dot",
    "Determinant",
    "MatrixRank",
    "Identity",
    "Dimension",
    "Invert",
    "Transpose",
    "ReducedRowEchelonForm",
    "ApplyMatrix",
    "Dilate",
    "Vector",
    "Direction",
    "UnitVector",
    "OrthogonalVector",
    "PerpendicularVector",
    "UnitOrthogonalVector",
    "UnitPerpendicularVector",
];

pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    match text {
        "Length" => p.list_command(length),
        "Distance" => p.list_command(|p, args, _| {
            let &[a, b] = args else { return Ok(None) };
            if [a, b]
                .iter()
                .any(|&i| matches!(p.node(i), Node::Local(_, Elem::Vec)))
                && [a, b].iter().all(|&i| p.is_point_or_vector(i))
            {
                let difference = p.push(Node::Bin(Bin::Sub, a, b));
                return Ok(Some(Node::Un(Un::Abs, difference)));
            }
            Ok(None)
        }),
        "Cross" => p.list_command(cross),
        "Dot" => p.list_command(dot),
        "Determinant" => p.list_command(|_, args, lists| {
            list_form(text, ListOp::Determinant, false, args, lists)
        }),
        "MatrixRank" => {
            p.list_command(|_, args, lists| list_form(text, ListOp::MatrixRank, false, args, lists))
        }
        "Dimension" => p.list_command(dimension),
        "Invert" => p.function_command(|p, args, lists| match (args, lists) {
            (&[a], [false]) if p.uses_function_variable(a) => Ok(Some(Node::Inverse(a))),
            _ => list_form(text, ListOp::Invert, false, args, lists),
        }),
        "Dilate" => p.function_command(|p, args, lists| {
            let (src, factor, center) = match *args {
                [src, factor] => (src, factor, NONE),
                [src, factor, center] => (src, factor, center),
                _ => return Ok(None),
            };
            Ok((lists[0] || p.uses_function_variable(src))
                .then_some(Node::Dilate(src, factor, center)))
        }),
        "Transpose" | "ReducedRowEchelonForm" => p.list_command(|p, args, lists| {
            let op = if text == "Transpose" {
                ListOp::Transpose
            } else {
                ListOp::Rref
            };
            if args.len() >= 2
                && lists.iter().all(|&list| !list)
                && (args.iter().all(|&arg| p.is_number(arg))
                    || args.iter().all(|&arg| p.is_point_or_vector(arg)))
            {
                return Ok(Some(Node::ListOp(op, p.wrap_list(args), NONE)));
            }
            list_form(text, op, false, args, lists)
        }),
        "ApplyMatrix" => p.list_command(|_, args, lists| {
            Ok(match (args, lists) {
                (&[matrix, src], [true, true]) => {
                    Some(Node::ListOp(ListOp::ApplyMatrix, matrix, src))
                }
                _ => None,
            })
        }),
        _ if VectorDef::vector_result(text) => {
            p.list_command(|p, args, lists| Ok(vector(p, text, args, lists)))
        }
        "Identity" => {
            let [n] = p.arguments()?[..] else {
                return Err("`Identity` takes one number".into());
            };
            Ok(Some(identity(p, n)))
        }
        _ => Ok(None),
    }
}

/// Inline coordinates use the same arithmetic in any expression. Named
/// inputs use the script algorithms, which also have their drawing start
/// points and raw line or intersection coordinates.
fn vector(p: &mut Parser, command: &str, args: &[u32], lists: &[bool]) -> Option<Node> {
    let (&[a], [false]) = (args, lists) else {
        return None;
    };
    if matches!(p.node(a), Node::Obj(_)) {
        return None;
    }
    let x = p.push(Node::Un(Un::X, a));
    let y = p.push(Node::Un(Un::Y, a));
    let perpendicular = matches!(
        command,
        "OrthogonalVector"
            | "PerpendicularVector"
            | "UnitOrthogonalVector"
            | "UnitPerpendicularVector"
    );
    let (x, y) = if perpendicular {
        let minus = p.push(Node::MinusOne);
        (p.push(Node::Bin(Bin::Mul, minus, y)), x)
    } else {
        (x, y)
    };
    let tuple = Node::Tuple(x, y);
    let output = if matches!(
        command,
        "UnitVector" | "UnitOrthogonalVector" | "UnitPerpendicularVector"
    ) {
        let pair = p.push(tuple);
        let len = p.push(Node::Un(Un::Abs, pair));
        p.push(Node::Bin(Bin::Div, pair, len))
    } else {
        p.push(tuple)
    };
    Some(Node::Vector(output))
}

fn dimension(p: &mut Parser, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    if let (&[a], [true]) = (args, lists) {
        let op = if p.is_matrix(a) {
            ListOp::MatrixDimension
        } else {
            ListOp::Dimension
        };
        return Ok(Some(Node::ListOp(op, a, NONE)));
    }
    let (&[a], [false]) = (args, lists) else {
        return Ok(None);
    };
    if matches!(p.node(a), Node::Obj(_)) {
        return Ok(None);
    }
    let x = p.push(Node::Un(Un::X, a));
    let y = p.push(Node::Un(Un::Y, a));
    let defined_x = p.push(Node::Bin(Bin::Equal, x, x));
    let defined_y = p.push(Node::Bin(Bin::Equal, y, y));
    let defined = p.push(Node::Bin(Bin::And, defined_x, defined_y));
    let two = p.push(Node::Num(2.0));
    Ok(Some(Node::If(IfMode::Algo, defined, two, NONE)))
}

fn length(p: &mut Parser, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    match (args, lists) {
        (&[a], [false]) if p.is_point_or_vector(a) => Ok(Some(Node::Un(Un::Abs, a))),
        _ => list_form("Length", ListOp::Length, false, args, lists),
    }
}

/// `ExpressionNodeEvaluator.vectorProduct` for two 2D inputs.
fn cross(p: &mut Parser, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    if let (&[a, b], [true, true]) = (args, lists) {
        return Ok(Some(Node::ListOp(ListOp::Cross, a, b)));
    }
    let (&[a, b], [false, false]) = (args, lists) else {
        return Err("`Cross` takes two points or vectors, or two lists".into());
    };
    let ax = p.push(Node::Un(Un::X, a));
    let ay = p.push(Node::Un(Un::Y, a));
    let bx = p.push(Node::Un(Un::X, b));
    let by = p.push(Node::Un(Un::Y, b));
    let left = p.push(Node::Bin(Bin::Mul, ax, by));
    let right = p.push(Node::Bin(Bin::Mul, ay, bx));
    Ok(Some(Node::Bin(Bin::Sub, left, right)))
}

fn dot(p: &mut Parser, args: &[u32], lists: &[bool]) -> Result<Option<Node>, String> {
    let &[a, b] = args else {
        return Err("`Dot` takes two arguments".into());
    };
    match lists {
        [true, false] => Err("`Dot` cannot multiply a list by a non-list on its right".into()),
        [true, true] => {
            if let (Node::Obj(a), Node::Obj(b)) = (p.node(a), p.node(b)) {
                return Ok(Some(Node::Cmd(
                    Def::Vector(VectorDef::DotLists(a, b)),
                    Hint::None,
                )));
            }
            Ok(Some(Node::ListOp(ListOp::Dot, a, b)))
        }
        _ => Ok(Some(Node::Bin(Bin::Mul, a, b))),
    }
}

/// Generate the diagonal with the existing bounded list builder.
fn identity(p: &mut Parser, n: u32) -> u32 {
    let one = p.push(Node::Num(1.0));
    let zero = p.push(Node::Num(0.0));
    let number = p.push(Node::Bin(Bin::Add, n, zero));
    let floor = p.push(Node::Un(Un::Floor, number));
    // Floor can snap up within the standard precision. Scale the distance
    // from its result so a representable value just below it still truncates.
    let delta = p.push(Node::Bin(Bin::Sub, number, floor));
    let scale = p.push(Node::Num(2.0_f64.powi(32)));
    let delta = p.push(Node::Bin(Bin::Mul, delta, scale));
    let rounded_up = p.push(Node::Bin(Bin::Less, delta, zero));
    let below = p.push(Node::Bin(Bin::Sub, floor, one));
    let size = p.push(Node::If(IfMode::Algo, rounded_up, below, floor));
    let depth = p.locals.len() as u32;
    let i = p.push(Node::Local(depth, Elem::Num));
    let j = p.push(Node::Local(depth + 1, Elem::Num));
    let on_diagonal = p.push(Node::Bin(Bin::Equal, i, j));
    let entry = p.push(Node::If(IfMode::Algo, on_diagonal, one, zero));
    let row = p.push(Node::Seq(entry, one, size, NONE, depth + 1));
    let matrix = p.push(Node::Seq(row, one, size, NONE, depth));
    p.push(Node::ListOp(ListOp::Identity, matrix, n))
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    elem: Elem,
    numbers: bool,
    _nodes: &[Node],
    tys: &[Ty],
    _l: u32,
    n: u32,
) -> Result<Option<Ty>, String> {
    Ok(match op {
        ListOp::Length
        | ListOp::Determinant
        | ListOp::MatrixRank
        | ListOp::Dot
        | ListOp::Dimension => Some(Ty::Num(0)),
        ListOp::MatrixDimension => Some(Ty::List(Elem::Num)),
        ListOp::Transpose | ListOp::Rref | ListOp::Invert | ListOp::Identity => {
            Some(Ty::List(Elem::List))
        }
        ListOp::ApplyMatrix => match tys[n as usize] {
            ty @ Ty::List(_) => Some(ty),
            _ => return Err("`ApplyMatrix` needs a list to transform".into()),
        },
        ListOp::Cross => {
            let Ty::List(right) = tys[n as usize] else {
                return Err("`Cross` needs two lists".into());
            };
            let right_numbers = matches!(right, Elem::Num | Elem::Bool | Elem::Empty | Elem::Any);
            let result = if numbers && right_numbers || elem == Elem::Vec && right == Elem::Vec {
                Elem::Num
            } else {
                Elem::Any
            };
            Some(Ty::List(result))
        }
        _ => None,
    })
}

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    _i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Length | ListOp::Dimension => {
            Val::N(ctx.list(l).map_or(f64::NAN, |(_, len)| f64::from(len)))
        }
        ListOp::Dot => Val::N(dot_lists(ctx, l, n).unwrap_or(f64::NAN)),
        ListOp::Determinant | ListOp::MatrixRank => {
            Val::N(matrix_number(ctx, op, l).unwrap_or(f64::NAN))
        }
        _ => return None,
    })
}

fn dot_lists<const M: bool>(ctx: &Ctx<'_, M>, left: u32, right: u32) -> Option<f64> {
    let lists = ctx.lists?;
    let (a, len) = ctx.list(left)?;
    let (b, other) = ctx.list(right)?;
    if len != other || !lists.charge(u64::from(len)) {
        return None;
    }
    (0..len).try_fold(0.0, |sum, k| {
        let x = number_of(lists.read(ctx.c, a + k))?;
        let y = number_of(lists.read(ctx.c, b + k))?;
        Some(sum + x * y)
    })
}

pub(super) fn makes_list(op: ListOp) -> bool {
    matches!(
        op,
        ListOp::Cross
            | ListOp::Transpose
            | ListOp::Rref
            | ListOp::Invert
            | ListOp::MatrixDimension
            | ListOp::Identity
            | ListOp::ApplyMatrix
    )
}

pub(super) fn list_fn<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<(u32, u32)> {
    if op == ListOp::Cross {
        return cross_lists(ctx, l, n);
    }
    let lists = ctx.lists?;
    if op == ListOp::Identity {
        if ctx.e.stuck.contains(l) {
            return None;
        }
        let size = ctx.full_number(n) as i32;
        if size <= 0 {
            if !lists.over() {
                ctx.e.stuck.insert(l);
            }
            return None;
        }
        return ctx.list(l);
    }
    let (start, len) = ctx.list(l)?;
    if op == ListOp::Transpose {
        if len == 0 {
            return Some((lists.len(), 0));
        }
        let (_, cols) = lists.read(ctx.c, start).run()?;
        if !lists.charge(u64::from(len) * (1 + u64::from(cols))) {
            return None;
        }
        for k in 0..len {
            if lists.read(ctx.c, start + k).run()?.1 != cols {
                return None;
            }
        }
        let out = lists.reserve(cols)?;
        for col in 0..cols {
            let column = lists.reserve(len)?;
            for row in 0..len {
                let (source, _) = lists.read(ctx.c, start + row).run()?;
                lists.set(column + row, lists.read(ctx.c, source + col));
            }
            lists.set(out + col, Slot::list(column, len));
        }
        return Some((out, cols));
    }
    if op == ListOp::ApplyMatrix {
        let m = lists.with_list(ctx.c, start, len, |list| Some(matrix::Matrix::read(list)))?;
        let source = ctx.list(n)?;
        return map_list(ctx, source, |value| m?.apply(value));
    }
    if op == ListOp::MatrixDimension {
        if len == 0 {
            return None;
        }
        let (_, cols) = lists.read(ctx.c, start).run()?;
        if !lists.charge(u64::from(len)) {
            return None;
        }
        for k in 1..len {
            if lists.read(ctx.c, start + k).run()?.1 != cols {
                return None;
            }
        }
        let out = lists.reserve(2)?;
        lists.set(out, Slot::number(len as f64));
        lists.set(out + 1, Slot::number(cols as f64));
        return Some((out, 2));
    }
    // The matrix may borrow the arena. Stage numbers outside it until the
    // borrow ends, then reserve and fill the result's runs.
    let (data, rows, cols) = lists.with_list(ctx.c, start, len, |m| {
        let charge = |work| lists.charge(work);
        let output = |data: &[f64], rows, cols| {
            charge(data.len() as u64).then(|| (data.to_vec(), rows as u32, cols as u32))
        };
        match op {
            ListOp::Rref => matrix::rref(m, charge, output),
            ListOp::Invert => matrix::inverse(m, charge, output),
            _ => unreachable!("no matrix list result for {op:?}"),
        }
    })?;
    let out = lists.reserve(rows)?;
    for (row, entries) in data.chunks_exact(cols as usize).enumerate() {
        let run = lists.reserve(cols)?;
        for (col, &cell) in entries.iter().enumerate() {
            lists.set(run + col as u32, Slot::number(cell));
        }
        lists.set(out + row as u32, Slot::list(run, cols));
    }
    Some((out, rows))
}

/// Transform leaves in depth-first order. A scalar is copied, while the
/// transformation reuses its last point or vector output.
pub(in crate::expr) fn map_list<const M: bool>(
    ctx: &Ctx<'_, M>,
    (start, len): (u32, u32),
    transform: impl Fn(Value) -> Option<Value>,
) -> Option<(u32, u32)> {
    let lists = ctx.lists?;
    let transformed = |slot: Slot| match transform(slot.value) {
        Some(point @ (Value::Point { .. } | Value::HomogeneousPoint { .. })) => Slot::point(point),
        Some(Value::Vector { x, y }) => Slot::coords(Kind::Vector, x, y),
        _ => Slot::undefined(slot.kind),
    };
    let out = lists.reserve(len)?;
    let mut pending = Vec::new();
    let mut current = (start, out, len);
    let mut last = None;
    loop {
        let (source, target, size) = current;
        if size == 0 {
            let Some(next) = pending.pop() else {
                return Some((out, len));
            };
            current = next;
            continue;
        }
        if !lists.charge(1) {
            return None;
        }
        current = (source + 1, target + 1, size - 1);
        let slot = lists.read(ctx.c, source);
        let result = if let Some((list, len)) = slot.run() {
            let dest = lists.reserve(len)?;
            if len != 0 {
                pending.push(current);
                current = (list, dest, len);
            }
            Slot::list(dest, len)
        } else if matches!(slot.kind, Kind::Point | Kind::Vector) {
            last = Some(target);
            transformed(slot)
        } else {
            if slot.kind != Kind::List {
                let previous = last?;
                lists.set(previous, transformed(lists.read(ctx.c, previous)));
            }
            slot
        };
        lists.set(target, result);
    }
}

fn cross_lists<const M: bool>(ctx: &Ctx<'_, M>, left: u32, right: u32) -> Option<(u32, u32)> {
    let lists = ctx.lists?;
    let (a, len) = ctx.list(left)?;
    let (b, other) = ctx.list(right)?;
    if len != other || len == 0 {
        return Some((lists.len(), 0));
    }
    if !matches!(len, 2 | 3) || !lists.charge(u64::from(len)) {
        return None;
    }
    let product = |i, j| {
        let x = slot_val(&lists.read(ctx.c, a + i));
        let y = slot_val(&lists.read(ctx.c, b + j));
        slot_val(&combine(Bin::Mul, x, y))
    };
    let component = |i, j| combine(Bin::Sub, product(i, j), product(j, i));
    let start = lists.reserve(3)?;
    let (x, y) = if len == 3 {
        (component(1, 2), component(2, 0))
    } else {
        (Slot::number(0.0), Slot::number(0.0))
    };
    for (k, value) in [x, y, component(0, 1)].into_iter().enumerate() {
        lists.set(start + k as u32, value);
    }
    Some((start, 3))
}

fn matrix_number<const M: bool>(ctx: &Ctx<'_, M>, op: ListOp, matrix: u32) -> Option<f64> {
    let lists = ctx.lists?;
    let (start, len) = ctx.list(matrix)?;
    lists.with_list(ctx.c, start, len, |matrix| {
        let charge = |work| lists.charge(work);
        if op == ListOp::Determinant {
            matrix::determinant(matrix, charge)
        } else {
            matrix::rank(matrix, charge)
        }
    })
}

impl Construction {
    /// Store an argument tree without creating a labelled construction object.
    pub(crate) fn store_intersection_argument(
        &mut self,
        text: &str,
    ) -> Result<(u32, Kind), String> {
        let parsed = crate::expr::parse::parse(self, text)?;
        let kind = match (parsed.uses_var, parsed.tys[parsed.root as usize]) {
            (false, Ty::Num(1)) => Kind::Angle,
            (false, Ty::Num(_)) => Kind::Number,
            (false, Ty::Vec) => Kind::Point,
            _ => return Err(format!("invalid intersection argument `{text}`")),
        };
        let index = u32::try_from(self.exprs.entries.len())
            .map_err(|_| "too many expression arguments".to_string())?;
        let entry = Entry::new(self, parsed, false);
        self.exprs.entries.push(entry);
        Ok((index, kind))
    }

    pub(crate) fn intersection_argument_parents(&self, index: u32) -> Option<&[Id]> {
        Some(&self.exprs.entries.get(index as usize)?.parents)
    }

    pub(crate) fn intersection_argument_value(&self, index: u32) -> Option<Raw> {
        let entry = self.exprs.entries.get(index as usize)?;
        let lists = entry.lists.then(|| expr::eval::Lists::new(Vec::new()));
        Some(expr::eval::root(self, entry, lists.as_ref()).0)
    }
}
