// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoStats1D.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The statistics group of the command matrix: `Sum` and `Mean`.

use crate::expr::eval::{number_of, slot_val, Ctx, Val};
use crate::expr::parse::{list_form, Parser};
use crate::expr::types::of_numbers;
use crate::expr::{Bin, Elem, ListOp, Node, Ty, NONE};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["Sum", "Mean"];

/// `Sum(list)`, `Sum(list, n)` and `Mean(list)`; of anything else they are
/// the commands of the script parser.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let (op, count) = match text {
        "Sum" => (ListOp::Sum, true),
        "Mean" => (ListOp::Mean, false),
        _ => return Ok(None),
    };
    p.list_command(|p, args, lists| {
        // `Sum(a, b, ...)` of anything but a list folds the arguments,
        // which GeoGebra wraps in a list of the class of the first
        // (`CmdSum`). Functions splice their bodies into one sum
        // (`FunctionFold`): applying them at a shared variable does the
        // same. Of two arguments the fold never runs: `CmdSum` wants a
        // list first (`argErr`).
        if op == ListOp::Sum && args.len() >= 3 && !lists[0] {
            let functions: Option<Vec<_>> = args.iter().map(|&a| p.applied_function(a)).collect();
            if let Some(ids) = functions {
                let var = p.push_var();
                let applied: Vec<u32> =
                    ids.iter().map(|&id| p.push(Node::Apply(id, var))).collect();
                let (last, init) = applied.split_last().unwrap();
                let mut acc = init[0];
                for &a in &init[1..] {
                    acc = p.push(Node::Bin(Bin::Add, acc, a));
                }
                return Ok(Some(Node::Bin(Bin::Add, acc, *last)));
            }
            let list = p.wrap_list(args);
            return Ok(Some(Node::ListOp(op, list, NONE)));
        }
        // `Sum(list, frequencies)`: the second argument is a list too
        if !(op == ListOp::Sum && args.len() == 2 && lists[0] && lists[1]) {
            return list_form(text, op, count, args, lists);
        }
        Ok(Some(Node::ListOp(op, args[0], args[1])))
    })
}

/// The type of a list command of this group (see `commands::list_type`).
pub(super) fn list_type(
    op: ListOp,
    elem: Elem,
    numbers: bool,
    _nodes: &[Node],
    _tys: &[Ty],
    _l: u32,
    _n: u32,
) -> Result<Option<Ty>, String> {
    Ok(Some(match op {
        // `AlgoStats1D` reads numbers; `Sum` adds points and vectors too
        ListOp::Sum if elem == Elem::Vec => Ty::Vec,
        ListOp::Sum | ListOp::Mean => of_numbers(numbers)?,
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
        ListOp::Sum if ctx.e.tys[i as usize] == Ty::Vec => ctx.sum_vectors(l, n),
        ListOp::Sum | ListOp::Mean => Val::N(ctx.stats(op, l, n)),
        _ => return None,
    })
}

impl<const M: bool> Ctx<'_, M> {
    /// `AlgoStats1D`: the sum, product or mean of the first `n` (all by
    /// default) elements as doubles. `n` is cut to a whole number and
    /// anything outside the list is undefined; no elements sum to 0, and
    /// have no product (as recorded with the harness) and no mean.
    pub(crate) fn stats(&self, op: ListOp, l: u32, n: u32) -> f64 {
        let Some(lists) = self.lists else {
            return f64::NAN;
        };
        // `Sum(list, frequencies)`: a second list weights the elements,
        // and a list one longer than its frequencies holds class bounds
        // whose midpoints count instead (`AlgoStats1D`).
        if op == ListOp::Sum && n != NONE && matches!(self.e.tys[n as usize], Ty::List(_)) {
            return self.sum_frequencies(lists, l, n);
        }
        let Some((start, size)) = self.truncated(l, n) else {
            return f64::NAN;
        };
        if size == 0 {
            return if op == ListOp::Sum { 0.0 } else { f64::NAN };
        }
        let (mut sum, mut product) = (0.0, 1.0);
        if !lists.charge(u64::from(size)) {
            return f64::NAN;
        }
        for k in 0..size {
            let Some(v) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            sum += v;
            product *= v;
        }
        match op {
            ListOp::Sum => sum,
            ListOp::Product => product,
            _ => sum / f64::from(size),
        }
    }

    /// `Sum(list, frequencies)` (`AlgoStats1D` with `STATS_SIGMAX`): the
    /// sum of each element times its frequency, with the midpoint rule for
    /// class bounds; lists of any other lengths, a negative frequency or a
    /// missing element make the sum undefined, and no elements sum to 0.
    fn sum_frequencies(&self, lists: &crate::expr::eval::Lists, l: u32, f: u32) -> f64 {
        let (Some((start, len)), Some((fstart, flen))) = (self.list(l), self.list(f)) else {
            return f64::NAN;
        };
        let classes = len == flen + 1;
        if !classes && len != flen {
            return f64::NAN;
        }
        let size = if classes { len - 1 } else { len };
        if size == 0 {
            return 0.0;
        }
        if !lists.charge(u64::from(2 * size)) {
            return f64::NAN;
        }
        let mut sum = 0.0;
        for k in 0..size {
            let Some(value) = number_of(lists.read(self.c, start + k)) else {
                return f64::NAN;
            };
            let value = if classes {
                match number_of(lists.read(self.c, start + k + 1)) {
                    Some(next) => (value + next) / 2.0,
                    None => return f64::NAN,
                }
            } else {
                value
            };
            let Some(frequency) = number_of(lists.read(self.c, fstart + k)) else {
                return f64::NAN;
            };
            if frequency < 0.0 {
                return f64::NAN;
            }
            sum += value * frequency;
        }
        sum
    }

    /// The list `l` and how many of its elements a truncation `n` keeps
    /// (`AlgoStats1D`): all without `n`, `(int) n` otherwise, where 0 is
    /// none and a count below 0 or past the end is undefined.
    fn truncated(&self, l: u32, n: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let truncate = (n != NONE).then(|| {
            let mark = lists.len();
            let t = self.full_number(n);
            lists.truncate(mark);
            t
        });
        let (start, len) = self.list(l)?;
        match truncate {
            None => Some((start, len)),
            Some(t) if t.is_nan() => None,
            // Java's `(int)`: towards zero, saturating
            Some(t) => match u32::try_from(t as i32) {
                Ok(t) if t <= len => Some((start, t)),
                _ => None,
            },
        }
    }

    /// `Sum` of points and vectors: the sum of their coordinates.
    pub(crate) fn sum_vectors(&self, l: u32, n: u32) -> Val {
        let undefined = Val::V(f64::NAN, f64::NAN);
        let Some(lists) = self.lists else {
            return undefined;
        };
        let Some((start, size)) = self.truncated(l, n).filter(|&(_, size)| size > 0) else {
            return undefined;
        };
        let (mut x, mut y) = (0.0, 0.0);
        if !lists.charge(u64::from(size)) {
            return undefined;
        }
        for k in 0..size {
            match slot_val(&lists.read(self.c, start + k)) {
                Val::V(a, b) => {
                    x += a;
                    y += b;
                }
                _ => return undefined,
            }
        }
        Val::V(x, y)
    }
}
