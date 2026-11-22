// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIf.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! The logical group of the command matrix: `If`.

use crate::expr::eval::{Ctx, Ex, Val};
use crate::expr::parse::Parser;
use crate::expr::{children, IfMode, Node, Ty, NONE};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["If"];

/// `If(...)`.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    if text != "If" {
        return Ok(None);
    }
    p.if_command().map(Some)
}

impl Parser<'_> {
    /// `If(c, a)`, `If(c, a, b)` and `If(c1, a1, c2, a2, ..., [else])`
    /// (`CmdIf`). With `x` in a condition it is the operation of a
    /// function (`IF`, `IF_ELSE`, `IF_LIST`); otherwise the command
    /// `AlgoIf`.
    pub(crate) fn if_command(&mut self) -> Result<u32, String> {
        let args = self.arguments()?;
        if args.len() < 2 {
            return Err("`If` needs a condition and a value".into());
        }
        let pairs = args.len() / 2;
        let conditions_read_x = (0..pairs).any(|k| self.reads(args[2 * k], true));
        let mode = match (conditions_read_x, args.len()) {
            (false, _) => IfMode::Algo,
            (true, 3) if !self.reads(args[0], false) => IfMode::Else,
            (true, _) => IfMode::List,
        };
        let mut rest = if args.len() % 2 == 1 {
            args[args.len() - 1]
        } else {
            NONE
        };
        for k in (0..pairs).rev() {
            rest = self.push(Node::If(mode, args[2 * k], args[2 * k + 1], rest));
        }
        Ok(rest)
    }

    /// Whether the subtree at `i` reads `x` (`variable`) or an object.
    fn reads(&self, i: u32, variable: bool) -> bool {
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            let node = self.node(j);
            let found = if variable {
                matches!(node, Node::Var)
            } else {
                matches!(node, Node::Obj(_) | Node::Apply(..) | Node::Cmd(..))
            };
            if found {
                return true;
            }
            stack.extend(children(&node));
        }
        false
    }
}

impl<const M: bool> Ctx<'_, M> {
    /// `AlgoIf.compute` and `Operation.IF`, `IF_ELSE`, `IF_LIST` (see
    /// [`IfMode`]). Only the chosen branch is evaluated: an untaken branch
    /// changes no value.
    #[inline(never)]
    pub(crate) fn conditional(&self, i: u32, mode: IfMode, cond: u32, then: u32, rest: u32) -> Ex {
        let (value, defined) = match self.full(cond).val {
            Val::B(v, d) => (v, d),
            _ => (false, false),
        };
        let ty = self.e.tys[i as usize];
        let undefined = || match ty {
            Ty::Vec => Ex::plain(Val::V(f64::NAN, f64::NAN)),
            // `GeoBoolean.setUndefined` only sets false
            Ty::Bool => Ex::plain(Val::B(false, true)),
            Ty::Num(_) | Ty::List(_) | Ty::Object => Ex::plain(Val::N(f64::NAN)),
        };
        if mode == IfMode::Else && !defined {
            return undefined();
        }
        let chosen = if value {
            then
        } else if rest != NONE {
            rest
        } else {
            return undefined();
        };
        let x = self.full(chosen);
        if mode != IfMode::Algo {
            // the operation hands on the branch's value itself
            return x;
        }
        // `AlgoIf.setResult`: an undefined branch makes the result
        // undefined; a number copies its value and exact decimal
        // (`GeoNumeric.set`) into a helper number
        match (ty, x.val) {
            (_, Val::N(v)) if v.is_nan() => undefined(),
            (Ty::Vec, Val::N(v)) => Ex::plain(Val::V(v, 0.0)),
            (Ty::Num(_), Val::N(v)) => {
                // `getNumber` keeps the decimal when the definition is
                // constant (`reuseDefinition`): the branch is a literal, a
                // constant expression or a free number that has its own
                let special = x.dec.is_some()
                    && match self.node(chosen) {
                        Node::Obj(_) => x.special,
                        _ => self.is_constant(chosen),
                    };
                Ex {
                    val: Val::N(v),
                    dec: x.dec,
                    special,
                    obj: true,
                }
            }
            (_, Val::B(b, _)) => Ex::plain(Val::B(b, true)),
            (_, val) => Ex::plain(val),
        }
    }

    /// `AlgoIf` of lists: the first branch whose condition is true (an
    /// undefined condition reads false), the else branch, or undefined
    /// without one. What the condition built is dropped.
    pub(crate) fn list_if(&self, cond: u32, then: u32, rest: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let mark = lists.len();
        let value = matches!(self.full(cond).val, Val::B(true, _));
        lists.truncate(mark);
        if value {
            self.list(then)
        } else if rest != NONE {
            self.list(rest)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
