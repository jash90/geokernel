// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoStats1D.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The statistics group of the command matrix: `Sum` and `Mean`.

use crate::expr::eval::{number_of, slot_val, Ctx, Val};
use crate::expr::parse::{list_form, Parser};
use crate::expr::types::of_numbers;
use crate::expr::{Elem, ListOp, Node, Ty, NONE};

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
    p.list_command(|_, args, lists| list_form(text, op, count, args, lists))
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
