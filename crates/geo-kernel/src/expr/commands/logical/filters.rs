// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCountIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoKeepIf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDefined.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::eval::{slot_val, Ctx, Local, Val};
use crate::expr::lex::Tok;
use crate::expr::parse::Parser;
use crate::expr::{Elem, Node, Ty};
use crate::kernel::Kind;

impl Parser<'_> {
    pub(super) fn filter_command(&mut self, keep: bool) -> Result<u32, String> {
        let commas = self.argument_commas();
        let (end_condition, variable, list_start) = match *commas.as_slice() {
            [comma] => (comma, "x".to_string(), comma + 1),
            [first, last] if last == first + 2 => {
                let variable = self
                    .name_at(first + 1)
                    .ok_or("the second filter argument must be a variable")?;
                (first, variable.to_owned(), last + 1)
            }
            _ => return Err("a filter needs a condition, an optional variable and a list".into()),
        };
        let start = self.pos;
        self.pos = list_start;
        let list = self.expression()?;
        self.expect(&Tok::RParen, ")")?;
        let end = self.pos;
        let elem = self
            .list_element_type(list)
            .ok_or("the filter needs a list")?;
        let explicit = commas.len() == 2;
        let elem = match elem {
            Elem::Empty => Elem::Num,
            Elem::Any => self.list_template_type(list).unwrap_or(Elem::Num),
            elem => elem,
        };
        self.pos = start;
        self.expect(&Tok::LParen, "(")?;
        let local = self.locals.len() as u32;
        let condition = if commas.len() == 1 && self.pos + 1 == end_condition {
            self.kind_at(self.pos, Kind::Function).map(|f| {
                self.pos += 1;
                self.read(f, true);
                let arg = self.push(Node::Local(local, elem));
                self.push(Node::Apply(f, arg))
            })
        } else {
            None
        };
        let condition = if let Some(condition) = condition {
            condition
        } else {
            self.locals.push((variable, elem));
            let result = self.expression();
            self.locals.pop();
            result?
        };
        if self.pos != end_condition {
            return Err("expected a filter condition".into());
        }
        self.pos = end;
        Ok(self.push(Node::Filter {
            keep,
            list,
            condition,
            local,
            elem,
            explicit,
        }))
    }
}

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn is_defined_argument(&self, arg: u32) -> bool {
        if matches!(self.e.tys[arg as usize], Ty::List(_)) {
            return self.list(arg).is_some();
        }
        if self.e.tys[arg as usize] == Ty::Text {
            return self.text(arg).is_some();
        }
        match self.full(arg).val {
            Val::N(x) => !x.is_nan(),
            Val::V(x, y) => x.is_finite() && y.is_finite(),
            Val::B(_, defined) => {
                defined
                    || !matches!(
                        self.node(arg),
                        Node::Algebra(crate::expr::AlgebraOp::IsPrime, ..)
                    )
            }
        }
    }

    pub(crate) fn filter(
        &self,
        list: u32,
        condition: u32,
        depth: u32,
        elem: Elem,
        explicit: bool,
        keep: bool,
    ) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let Some((start, len)) = self.list(list) else {
            return (!explicit && !keep).then_some((0, 0));
        };
        let out = if keep { lists.reserve(len)? } else { 0 };
        let mut count = 0;
        for i in 0..len {
            if !lists.charge(1) {
                return None;
            }
            let slot = lists.read(self.c, start + i);
            let raw = slot_val(&slot);
            let value = match (elem, slot.kind, raw) {
                (Elem::Num, Kind::Number | Kind::Angle, _) => raw,
                // The two-argument form substitutes each value directly.
                // The explicit variable keeps the type of the list's template.
                (Elem::Num, Kind::Boolean, _) | (Elem::Bool, Kind::Number | Kind::Angle, _)
                    if !explicit =>
                {
                    raw
                }
                (Elem::Num, Kind::Boolean, Val::B(b, _)) => Val::N(f64::from(u8::from(b))),
                (Elem::Num, _, _) if explicit => Val::N(f64::NAN),
                (Elem::Vec, Kind::Point | Kind::Vector, _) => raw,
                (Elem::Vec, Kind::Number | Kind::Angle, Val::N(x)) if explicit => Val::V(x, 0.0),
                (Elem::Bool, Kind::Boolean, _) | (Elem::List, Kind::List, _) => raw,
                _ => return None,
            };
            let local = Local {
                depth,
                value,
                run: slot.run(),
                vector: slot.kind == Kind::Vector,
                up: self.locals,
            };
            let ctx = Ctx {
                locals: Some(&local),
                ..*self
            };
            let mark = lists.len();
            let tested = ctx.full(condition);
            lists.truncate(mark);
            if matches!(tested.val, Val::B(true, _)) {
                if keep {
                    lists.set(out + count, slot);
                }
                count += 1;
            }
        }
        Some((out, count))
    }
}
