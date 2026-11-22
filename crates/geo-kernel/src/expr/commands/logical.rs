// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIf.java
//   source/shared/common/src/main/java/org/geogebra/common/plugin/Operation.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoNumeric.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Conditionals, list filters and definedness.

mod filters;
pub(crate) mod relation;

use crate::expr::eval::{Ctx, Ex, Val};
use crate::expr::lex::Tok;
use crate::expr::parse::Parser;
use crate::expr::{children, IfMode, Node, Ty, NONE};
use crate::script::split_call;
use crate::{Construction, Def, Id, Kind, Value};

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["If", "CountIf", "KeepIf", "Defined", "IsDefined"];

pub(crate) fn conditional_kind(kind: Kind) -> Option<&'static [Kind]> {
    Some(match kind {
        Kind::Line => &[Kind::Line],
        Kind::Segment => &[Kind::Segment],
        Kind::Ray => &[Kind::Ray],
        Kind::Circle => &[Kind::Circle],
        Kind::Conic => &[Kind::Conic],
        _ => return None,
    })
}

/// `AlgoIf` where the alternatives are named geometric objects of one kind.
pub(crate) fn define_geometry_if(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    let ("If", args) = split_call(input)? else {
        return None;
    };
    let kind = c.kind(c.id(args.get(1)?)?);
    conditional_kind(kind)?;
    let checkpoint = c.checkpoint();
    let result = (|| {
        let mut parents = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let is_condition = index % 2 == 0 && index + 1 < args.len();
            let expected = if is_condition { Kind::Boolean } else { kind };
            let id = if let Some(id) = c.id(arg).filter(|&id| c.kind(id) == expected) {
                id
            } else if is_condition {
                let label = format!("\0arg{}", c.len());
                if !crate::expr::argument_object(c, arg, &label, Kind::Boolean) {
                    return Err(format!("`If` needs a boolean condition, got `{arg}`"));
                }
                c.id(&label).expect("argument object was just added")
            } else {
                return Err(format!(
                    "`If` needs a named {kind} alternative, got `{arg}`"
                ));
            };
            parents.push(id);
        }
        c.add_with_list(label, &parents, |args| Def::Conditional { args, kind })
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        c.discard_since(checkpoint);
    }
    Some(result)
}

pub(crate) fn conditional_branch(c: &Construction, ids: &[Id]) -> Option<Id> {
    let (pairs, fallback) = ids.as_chunks::<2>();
    pairs
        .iter()
        .find_map(|&[condition, branch]| {
            (c.value(condition) == Value::Bool(true)).then_some(branch)
        })
        .or_else(|| fallback.first().copied())
}

/// `If(...)`.
pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    match text {
        "If" => p.if_command().map(Some),
        "CountIf" | "KeepIf" => p.filter_command(text == "KeepIf").map(Some),
        "Defined" | "IsDefined" => {
            if matches!(p.toks.get(p.pos + 2).map(|t| &t.tok), Some(Tok::RParen)) {
                if let Some(id) = p.object_at(p.pos + 1) {
                    p.pos += 3;
                    p.read(id, true);
                    return Ok(Some(p.push(Node::ObjectDefined(id))));
                }
            }
            let [arg] = p.arguments()?[..] else {
                return Err(format!("`{text}` takes one argument"));
            };
            Ok(Some(p.push(Node::Defined(arg))))
        }
        _ => Ok(None),
    }
}

impl Parser<'_> {
    /// `If(c, a)`, `If(c, a, b)` and `If(c1, a1, c2, a2, ..., [else])`
    /// (`CmdIf`). With `x` in a condition it is the operation of a
    /// function (`IF`, `IF_ELSE`, `IF_LIST`); otherwise the command
    /// `AlgoIf`.
    pub(crate) fn if_command(&mut self) -> Result<u32, String> {
        let args = self.parse_arguments(true)?;
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
        let mut seen = vec![false; i as usize + 1];
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            if std::mem::replace(&mut seen[j as usize], true) {
                continue;
            }
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
            Ty::Num(_) | Ty::List(_) | Ty::Function | Ty::Text | Ty::Object => {
                Ex::plain(Val::N(f64::NAN))
            }
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
