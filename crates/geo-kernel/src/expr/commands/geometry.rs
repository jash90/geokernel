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

use crate::algo::intersect::Hint;
use crate::algo::vector::{matrix, VectorDef};
use crate::expr::eval::{combine, number_of, slot_val, Ctx, Env, Val};
use crate::expr::list::Slot;
use crate::expr::parse::{list_form, Parser};
use crate::expr::{self, math, Bin, Elem, IfMode, ListOp, Node, Ty, Un, NONE};
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
    // An inline coordinate pair belongs to this command. Even with literal
    // arguments the result is dependent and cannot be dragged as a free vector.
    let yes = p.push(Node::Bool(true));
    Some(Node::If(IfMode::Algo, yes, output, NONE))
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
    if let (&[a], [false]) = (args, lists) {
        if matches!(p.node(a), Node::Cmd(def, _) if def.kind() == Kind::Vector) {
            return Ok(Some(Node::Un(Un::Abs, a)));
        }
    }
    list_form("Length", ListOp::Length, false, args, lists)
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
    let n = p.push(Node::Bin(Bin::Add, n, zero));
    let floor = p.push(Node::Un(Un::Floor, n));
    // Floor can snap up within the standard precision. Scale the distance
    // from its result so a representable value just below it still truncates.
    let delta = p.push(Node::Bin(Bin::Sub, n, floor));
    let scale = p.push(Node::Num(2.0_f64.powi(32)));
    let delta = p.push(Node::Bin(Bin::Mul, delta, scale));
    let rounded_up = p.push(Node::Bin(Bin::Less, delta, zero));
    let below = p.push(Node::Bin(Bin::Sub, floor, one));
    let size = p.push(Node::If(IfMode::Algo, rounded_up, below, floor));
    let nonempty = p.push(Node::Bin(Bin::GreaterEqual, size, one));
    let depth = p.locals.len() as u32;
    let i = p.push(Node::Local(depth));
    let j = p.push(Node::Local(depth + 1));
    let on_diagonal = p.push(Node::Bin(Bin::Equal, i, j));
    let entry = p.push(Node::If(IfMode::Algo, on_diagonal, one, zero));
    let row = p.push(Node::Seq(entry, one, size, NONE, depth + 1));
    let matrix = p.push(Node::Seq(row, one, size, NONE, depth));
    p.push(Node::If(IfMode::Algo, nonempty, matrix, NONE))
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
        ListOp::Transpose | ListOp::Rref | ListOp::Invert => Some(Ty::List(Elem::List)),
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
        return map_list(ctx, source, |slot| m?.apply(slot.value));
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

/// Transform each leaf recursively while preserving the shape of the list.
pub(in crate::expr) fn map_list<const M: bool>(
    ctx: &Ctx<'_, M>,
    (start, len): (u32, u32),
    transform: impl Fn(Slot) -> Option<Value>,
) -> Option<(u32, u32)> {
    let lists = ctx.lists?;
    let mut pending = Vec::new();
    let out = lists.reserve(len)?;
    let mut run = (start, len, out);
    loop {
        let (source, size, target) = run;
        if !lists.charge(u64::from(size)) {
            return None;
        }
        for k in 0..size {
            let slot = lists.read(ctx.c, source + k);
            let result = if let Some((list, len)) = slot.run() {
                let dest = lists.reserve(len)?;
                if len != 0 {
                    pending.push((list, len, dest));
                }
                Slot::list(dest, len)
            } else {
                match transform(slot) {
                    Some(point @ (Value::Point { .. } | Value::HomogeneousPoint { .. })) => {
                        Slot::point(point)
                    }
                    Some(Value::Vector { x, y }) => Slot::coords(Kind::Vector, x, y),
                    _ => Slot::undefined(slot.kind),
                }
            };
            lists.set(target + k, result);
        }
        let Some(next) = pending.pop() else {
            return Some((out, len));
        };
        run = next;
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
        let x = slot_val(lists.read(ctx.c, a + i));
        let y = slot_val(lists.read(ctx.c, b + j));
        slot_val(combine(Bin::Mul, x, y))
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

type Polynomial = [f64; 5];

impl Construction {
    /// Coefficients in ascending degree for a polynomial function of degree
    /// at most four. Coefficients can read other construction objects.
    pub(crate) fn function_polynomial(&self, id: Id) -> Option<Polynomial> {
        let ctx = function_context(self, id, &Env::None)?;
        let p = polynomial(&ctx, ctx.e.root)?;
        p.iter().all(|x| x.is_finite()).then_some(p)
    }

    /// The derivative at a seed for Newton iteration, when the expression
    /// tree consists of operations with scalar differentiation rules.
    pub(crate) fn function_derivative(&self, id: Id, x: f64) -> Option<f64> {
        let env = Env::Value(x);
        let ctx = function_context(self, id, &env)?;
        derivative(&ctx, ctx.e.root).filter(|d| d.is_finite())
    }
}

fn function_context<'a>(
    c: &'a Construction,
    id: Id,
    env: &'a Env<'a, false>,
) -> Option<Ctx<'a, false>> {
    let (Def::Expr(def), Value::Function) = (c.def(id), c.value(id)) else {
        return None;
    };
    Some(Ctx {
        c,
        e: expr::entry(c, def),
        env,
        lists: None,
        meter: None,
        locals: None,
    })
}

fn scalar_polynomial(x: f64) -> Polynomial {
    [x, 0.0, 0.0, 0.0, 0.0]
}

fn product(a: Polynomial, b: Polynomial) -> Option<Polynomial> {
    let mut out = [0.0; 5];
    for (i, x) in a.into_iter().enumerate().filter(|&(_, x)| x != 0.0) {
        for (j, y) in b.into_iter().enumerate().filter(|&(_, y)| y != 0.0) {
            *out.get_mut(i + j)? += x * y;
        }
    }
    Some(out)
}

fn has_variable(ctx: &Ctx<'_, false>, i: u32) -> bool {
    let node = ctx.node(i);
    matches!(node, Node::Var) || expr::children(&node).any(|child| has_variable(ctx, child))
}

fn polynomial(ctx: &Ctx<'_, false>, i: u32) -> Option<Polynomial> {
    if !has_variable(ctx, i) {
        return Some(scalar_polynomial(ctx.full_number(i)));
    }
    match ctx.node(i) {
        Node::Var => Some([0.0, 1.0, 0.0, 0.0, 0.0]),
        Node::Bin(op, a, b) => {
            let a = polynomial(ctx, a)?;
            let b = polynomial(ctx, b)?;
            let b_is_scalar = b[1..].iter().all(|&x| x == 0.0);
            match op {
                Bin::Add => Some(std::array::from_fn(|i| a[i] + b[i])),
                Bin::Sub => Some(std::array::from_fn(|i| a[i] - b[i])),
                Bin::Mul => product(a, b),
                Bin::Div if b_is_scalar => Some(a.map(|x| x / b[0])),
                Bin::Pow if b_is_scalar && (0.0..=4.0).contains(&b[0]) && b[0].fract() == 0.0 => (0
                    ..b[0] as usize)
                    .try_fold(scalar_polynomial(1.0), |power, _| product(power, a)),
                _ => None,
            }
        }
        Node::Apply(f, arg) => {
            let a = polynomial(ctx, arg)?;
            let p = ctx.c.function_polynomial(f)?;
            let degree = p.iter().rposition(|&x| x != 0.0).unwrap_or(0);
            let mut out = scalar_polynomial(p[degree]);
            for &constant in p[..degree].iter().rev() {
                out = product(out, a)?;
                out[0] += constant;
            }
            Some(out)
        }
        _ => None,
    }
}

fn derivative(ctx: &Ctx<'_, false>, i: u32) -> Option<f64> {
    if !has_variable(ctx, i) {
        return Some(0.0);
    }
    Some(match ctx.node(i) {
        Node::Var => 1.0,
        Node::Apply(f, arg) => {
            derivative(ctx, arg)? * ctx.c.function_derivative(f, ctx.full_number(arg))?
        }
        Node::Bin(op, a, b) => {
            let (da, db) = (derivative(ctx, a)?, derivative(ctx, b)?);
            match op {
                Bin::Add => da + db,
                Bin::Sub => da - db,
                _ => {
                    let (a, b) = (ctx.full_number(a), ctx.full_number(b));
                    match op {
                        Bin::Mul => da * b + a * db,
                        Bin::Div => (da * b - a * db) / (b * b),
                        Bin::Pow if db == 0.0 => b * a.powf(b - 1.0) * da,
                        Bin::Pow => a.powf(b) * (db * a.ln() + b * da / a),
                        _ => return None,
                    }
                }
            }
        }
        Node::Un(op, arg) => {
            let x = ctx.full_number(arg);
            let factor = match op {
                Un::Sin => math::cos(x),
                Un::Cos => -math::sin(x),
                Un::Tan => 1.0 / math::cos(x).powi(2),
                Un::Sinh => x.cosh(),
                Un::Cosh => x.sinh(),
                Un::Tanh => 1.0 / x.cosh().powi(2),
                Un::Asin => 1.0 / (1.0 - x * x).sqrt(),
                Un::Acos => -1.0 / (1.0 - x * x).sqrt(),
                Un::Atan => 1.0 / (1.0 + x * x),
                Un::Sqrt => 0.5 / x.sqrt(),
                Un::Cbrt => 1.0 / (3.0 * x.cbrt().powi(2)),
                Un::Abs => x / x.abs(),
                Un::Exp => x.exp(),
                Un::Ln => x.recip(),
                Un::Log2 => 1.0 / (x * std::f64::consts::LN_2),
                Un::Log10 => 1.0 / (x * std::f64::consts::LN_10),
                _ => return None,
            };
            factor * derivative(ctx, arg)?
        }
        _ => return None,
    })
}
