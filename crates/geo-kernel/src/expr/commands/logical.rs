// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdDefined.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDefined.java
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

/// `AlgoDefined` uses an object's defined flag. For functions it also
/// checks the value string. Projective points at infinity and vectors with
/// infinite coordinates return false.
pub(in crate::expr) fn defined_object<const M: bool>(ctx: &Ctx<'_, M>, id: Id) -> Option<bool> {
    let c = ctx.c;
    Some(match c.kind(id) {
        Kind::Point => {
            c.stored_value(id).point_coords().is_some() && c.point_at_infinity(id).is_none()
        }
        Kind::Vector => matches!(c.value(id), Value::Vector { x, y }
            if x.is_finite() && y.is_finite()),
        Kind::Ray => c.raw_carrier(id).is_some(),
        Kind::Arc | Kind::Sector => c.stored_value(id).is_defined(),
        Kind::Function if crate::expr::object_defined(c, id) => {
            return super::text::function_has_value(ctx, id);
        }
        _ => crate::expr::object_defined(c, id),
    })
}

/// The commands of this group (see `commands::is_command`).
pub(super) const NAMES: &[&str] = &["If", "CountIf", "KeepIf", "Defined", "IsDefined"];
pub(crate) const MAX_IF_ALTERNATIVES: usize = 64;

fn alternative_count(args: usize) -> usize {
    args.div_ceil(2)
}

pub(crate) fn conditional_kind(kind: Kind) -> Option<&'static [Kind]> {
    Some(match kind {
        // `GeoLine.set` takes the coefficients of a segment, and
        // `GeoSegment.set` those of a line (`TestGeo.canSet` keeps the
        // class of the first alternative of the two).
        Kind::Line => &[Kind::Line, Kind::Segment],
        Kind::Segment => &[Kind::Segment, Kind::Line],
        Kind::Ray => &[Kind::Ray],
        Kind::Circle => &[Kind::Circle],
        Kind::Conic => &[Kind::Conic],
        Kind::Arc => &[Kind::Arc],
        Kind::Sector => &[Kind::Sector],
        Kind::Polygon => &[Kind::Polygon],
        Kind::PolyLine => &[Kind::PolyLine],
        Kind::Curve => &[Kind::Curve],
        Kind::Locus => &[Kind::Locus],
        _ => return None,
    })
}

/// Conditional alternatives that carry geometric values or records.
pub(crate) fn define_geometry_if(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    let ("If", args) = split_call(input)? else {
        return None;
    };
    if alternative_count(args.len()) > MAX_IF_ALTERNATIVES {
        return Some(Err("too many If alternatives".into()));
    }
    let first = args.get(1)?;
    let kind = c.id(first).map(|id| c.kind(id)).or_else(|| {
        crate::script::parse_def(c, first)
            .ok()
            .map(|def| def.kind())
    })?;
    let accepted = conditional_kind(kind)?;
    let checkpoint = c.checkpoint();
    let result = (|| {
        let mut parents = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let is_condition = index % 2 == 0 && index + 1 < args.len();
            let expected: &[Kind] = if is_condition {
                &[Kind::Boolean]
            } else {
                accepted
            };
            let id = if let Some(id) = c.id(arg).filter(|&id| expected.contains(&c.kind(id))) {
                id
            } else if is_condition {
                let label = format!("\0arg{}", c.len());
                if !crate::expr::argument_object(c, arg, &label, Kind::Boolean) {
                    return Err(format!("`If` needs a boolean condition, got `{arg}`"));
                }
                c.id(&label).expect("argument object was just added")
            } else {
                let def = crate::script::parse_def(c, arg)
                    .ok()
                    .filter(|def| accepted.contains(&def.kind()))
                    .ok_or_else(|| format!("`If` needs a {kind} alternative, got `{arg}`"))?;
                let label = format!("\0arg{}", c.len());
                c.add(&label, def).map_err(|e| e.to_string())?
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

pub(crate) fn conditional_branch(c: &Construction, ids: &[Id], end: u64) -> Option<Id> {
    if alternative_count(ids.len()) > MAX_IF_ALTERNATIVES {
        return None;
    }
    let (pairs, fallback) = ids.as_chunks::<2>();
    for &[condition, branch] in pairs {
        if !crate::expr::charge_carrier(end) {
            return None;
        }
        if c.value(condition) == Value::Bool(true) {
            return Some(branch);
        }
    }
    fallback
        .first()
        .copied()
        .filter(|_| crate::expr::charge_carrier(end))
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
            // A free x in this argument creates a function for CmdDefined;
            // it does not make the enclosing Boolean a function.
            let uses_var = std::mem::take(&mut p.uses_var);
            let [arg] = p.arguments()?[..] else {
                return Err(format!("`{text}` takes one argument"));
            };
            let function = p.uses_var;
            p.uses_var = uses_var;
            let node = match p.node(arg) {
                Node::Obj(id) | Node::FunctionObject(id) => Node::ObjectDefined(id),
                _ => Node::Defined(arg, function),
            };
            Ok(Some(p.push(node)))
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
        if alternative_count(args.len()) > MAX_IF_ALTERNATIVES {
            return Err("too many If alternatives".into());
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
            let mode = if mode == IfMode::Algo && k != 0 {
                IfMode::AlgoNext
            } else {
                mode
            };
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
    /// Select the original alternative so it is copied into the output
    /// type of the whole command just once.
    pub(crate) fn if_alternative(&self, mut i: u32) -> Option<u32> {
        for _ in 0..MAX_IF_ALTERNATIVES {
            let Node::If(mode, cond, then, rest) = self.node(i) else {
                return None;
            };
            // The metered condition evaluation charges every visited node.
            if self.meter.is_none() {
                crate::expr::charge(1);
            }
            let (value, defined) = match self.full(cond).val {
                Val::B(v, d) => (v, d),
                _ => (false, false),
            };
            if self.stop(0) {
                return None;
            }
            if mode == IfMode::Else && !defined {
                return None;
            }
            if value {
                return Some(then);
            }
            if rest == NONE {
                return None;
            }
            if !matches!(self.node(rest), Node::If(IfMode::AlgoNext, ..)) {
                return Some(rest);
            }
            i = rest;
        }
        None
    }

    /// `AlgoIf.compute` and `Operation.IF`, `IF_ELSE`, `IF_LIST` (see
    /// [`IfMode`]). Only the chosen branch is evaluated: an untaken branch
    /// changes no value.
    #[inline(never)]
    pub(crate) fn conditional(&self, i: u32, mode: IfMode) -> Ex {
        let ty = self.e.tys[i as usize];
        let undefined = || match ty {
            Ty::Vec => Ex::plain(Val::V(f64::NAN, f64::NAN)),
            // `GeoBoolean.setUndefined` only sets false
            Ty::Bool => Ex::plain(Val::B(false, true)),
            Ty::Num(_) | Ty::List(_) | Ty::Function | Ty::Text | Ty::Object => {
                Ex::plain(Val::N(f64::NAN))
            }
        };
        let Some(chosen) = self.if_alternative(i) else {
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
            (Ty::Num(_), Val::V(..)) => undefined(),
            (Ty::Bool, Val::N(v)) => Ex::plain(Val::B((v - 1.0).abs() < 1e-8, true)),
            (Ty::Num(_), Val::B(b, _)) => Ex::plain(Val::N(f64::from(u8::from(b)))),
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

    /// GeoList.getGenericElement supplies the copy AlgoIf starts with.
    /// For a list output it walks the alternatives while they are lists.
    pub(crate) fn if_list_template(&self, mut i: u32) -> Option<u32> {
        let mut last = None;
        for _ in 0..MAX_IF_ALTERNATIVES {
            let Node::If(_, _, branch, rest) = self.node(i) else {
                break;
            };
            if !self.is_list(branch) {
                break;
            }
            last = Some(branch);
            if rest == NONE {
                break;
            }
            if !matches!(self.node(rest), Node::If(IfMode::AlgoNext, ..)) {
                return if self.is_list(rest) { Some(rest) } else { last };
            }
            i = rest;
        }
        last
    }

    /// `AlgoIf` of lists: the first branch whose condition is true (an
    /// undefined condition reads false), the else branch, or undefined
    /// without one. What the condition built is dropped.
    pub(crate) fn list_if(&self, i: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let mark = lists.len();
        let chosen = self.if_alternative(i);
        lists.truncate(mark);
        let node = self.node(i);
        if let Node::If(IfMode::Algo, test, branch, _) = node {
            // A literal false can never select its branch. If the If needs
            // that branch as its initial template, list() reads it there.
            let inactive = matches!(self.node(test), Node::Bool(false)).then_some(branch);
            lists.update_helpers(|| {
                for child in children(&node).filter(|&child| Some(child) != inactive) {
                    self.update_unchosen_helpers(child, chosen);
                }
            });
        }
        self.list(chosen?)
    }

    // Inline list helpers are independently updated inputs of AlgoIf.
    // Traverse the unchosen alternatives without evaluating ordinary operations.
    fn update_unchosen_helpers(&self, node: u32, chosen: Option<u32>) {
        if Some(node) == chosen || self.stop(1) {
            return;
        }
        match self.node(node) {
            Node::Call(crate::expr::CallOp::Take, ..) | Node::If(IfMode::Algo, ..)
                if self.is_list(node) =>
            {
                let lists = self.lists.expect("list evaluation");
                let mark = lists.len();
                self.list(node);
                lists.truncate(mark);
            }
            // These expressions are evaluated with locally bound arguments.
            Node::Seq(..) | Node::Filter { .. } => {}
            node => {
                for child in children(&node) {
                    self.update_unchosen_helpers(child, chosen);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
