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
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The list group of the command matrix: `Sequence`, `Element` (and
//! `l(n)`), `Product`, `Reverse`, `First`, `Last`, `Sort`, `Append` and
//! `Join`.

use crate::expr::eval::{count, slot_val, work, Ctx, Local, Val, OBJECT_SHIFT};
use crate::expr::lex::Tok;
use crate::expr::list::Slot;
use crate::expr::math;
use crate::expr::parse::{list_arity, list_form, Parser};
use crate::expr::types::{elem_of, join_elem, of_numbers, ty_of};
use crate::expr::{Elem, ListOp, Node, Ty, NONE};
use crate::kernel::Kind;

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &[
    "Sequence", "Element", "Product", "Reverse", "First", "Last", "Sort", "Append", "Join",
];

/// `Sequence` and `Element`, and the other commands of a list (of anything
/// else they are the commands of the script parser).
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let (op, count) = match text {
        "Sequence" => return p.sequence().map(Some),
        "Element" => {
            return match p.arguments()?[..] {
                [l, n] => Ok(Some(p.push(Node::ListOp(ListOp::Element, l, n)))),
                _ => Err("`Element` takes a list and one index".into()),
            }
        }
        "Append" => return p.list_command(|_, args, lists| append(text, args, lists)),
        "Join" => return p.list_command(|p, args, lists| join(p, text, args, lists)),
        "Product" => (ListOp::Product, true),
        "First" => (ListOp::First, true),
        "Last" => (ListOp::Last, true),
        "Reverse" => (ListOp::Reverse, false),
        "Sort" => (ListOp::Sort, false),
        _ => return Ok(None),
    };
    p.list_command(|_, args, lists| list_form(text, op, count, args, lists))
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
    )
}

/// Whether the second operand of `op` is an object.
pub(super) fn takes_object(op: ListOp) -> bool {
    matches!(op, ListOp::Append | ListOp::Prepend)
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
        ListOp::Product => of_numbers(numbers)?,
        ListOp::Index | ListOp::Element => ty_of(elem)?,
        ListOp::Reverse | ListOp::First | ListOp::Last | ListOp::Sort => Ty::List(elem),
        ListOp::Append | ListOp::Prepend => Ty::List(join_elem(elem, elem_of(tys[n as usize]))),
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

/// The value of a list command of this group (see `commands::list_value`).
pub(super) fn list_value<const M: bool>(
    ctx: &Ctx<'_, M>,
    i: u32,
    op: ListOp,
    l: u32,
    n: u32,
) -> Option<Val> {
    Some(match op {
        ListOp::Product => Val::N(ctx.stats(op, l, n)),
        // a list: built by `list`
        ListOp::Reverse
        | ListOp::First
        | ListOp::Last
        | ListOp::Sort
        | ListOp::Append
        | ListOp::Prepend
        | ListOp::Join => Val::N(f64::NAN),
        ListOp::Index | ListOp::Element => match (ctx.pick(op, l, n), ctx.e.tys[i as usize]) {
            (Some(slot), _) => slot_val(slot),
            (None, Ty::Vec) => Val::V(f64::NAN, f64::NAN),
            (None, Ty::Bool) => Val::B(false, true),
            (None, _) => Val::N(f64::NAN),
        },
        _ => return None,
    })
}

/// `Kernel.MIN_PRECISION`, the slack of the bounds of `Sequence`.
const MIN_PRECISION: f64 = 1e-5;

/// `MyMath.LARGEST_INTEGER`.
const LARGEST_INTEGER: f64 = 9007199254740992.0;

impl Parser<'_> {
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
        self.locals.push(variable);
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
                value: v,
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
        (0..i64::from(len))
            .contains(&k)
            .then(|| lists.read(self.c, start + k as u32))
    }

    /// A list command whose result is a list, built at the end of the
    /// scratch buffer.
    pub(crate) fn list_fn(&self, op: ListOp, l: u32, n: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
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
                if !matches!(
                    self.e.tys[l as usize],
                    Ty::List(Elem::Num | Elem::Bool | Elem::Empty)
                ) {
                    // AlgoFirst and AlgoLast copy each selected object.
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
            ListOp::Sort => {
                let (s, len) = self.list(l)?;
                let first = (len > 0).then(|| lists.read(self.c, s).kind)?;
                if !lists.charge(u64::from(len) * u64::from(1 + len.ilog2())) {
                    return None;
                }
                // numbers or booleans (`GeoNumeric.getComparator`) or
                // points (`GeoPoint.getComparatorX`), all of the same class
                if !matches!(first, Kind::Number | Kind::Boolean | Kind::Point)
                    || (0..len).any(|k| lists.read(self.c, s + k).kind != first)
                {
                    return None;
                }
                let (start, len) = self.copy_run(s, len)?;
                lists.sort(start, len);
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
