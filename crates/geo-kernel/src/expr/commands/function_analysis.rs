// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIteration.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIterationList.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRootList.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::{
    eval::{number_of, slot_val, Ctx, Env, Local, Val, MAX_ELEMENTS},
    lex::Tok,
    parse::{list_form, Parser},
    types::{elem_of, ty_of},
    Elem, IterationMode, ListOp, Node, Slot, Ty, NONE,
};
use crate::{Kind, Value};

pub(super) const NAMES: &[&str] = &[
    "RootList",
    "Iteration",
    "IterationList",
    "NInvert",
    "nroot",
    "Asymptote",
    "Normalize",
];

pub(super) fn parse(p: &mut Parser<'_>, name: &str) -> Result<Option<u32>, String> {
    let node = match name {
        "Normalize" => {
            return p.list_command(|p, args, lists| {
                if !args.is_empty() && args.iter().all(|&a| point_argument(p, a)) {
                    let list = p.wrap_list(args);
                    return Ok(Some(Node::ListOp(ListOp::Normalize, list, NONE)));
                }
                list_form(name, ListOp::Normalize, false, args, lists)
            });
        }
        "nroot" => {
            let args = p.arguments()?;
            let [number, degree] = args[..] else {
                return Err("nroot needs a number and degree".into());
            };
            let one = p.push(Node::Num(1.0));
            let exponent = p.push(Node::Bin(crate::expr::Bin::Div, one, degree));
            Node::Bin(crate::expr::Bin::Pow, number, exponent)
        }
        "Asymptote" => {
            let uses_var = p.uses_var;
            let result = p.function_command(|p, args, _| match args {
                [a] if matches!(p.node(*a), Node::Apply(_, x) if matches!(p.node(x), Node::Var)) => {
                    Ok(Some(Node::FunctionAsymptotes(*a)))
                }
                _ => Ok(None),
            });
            p.uses_var = uses_var;
            return result;
        }
        "NInvert" => {
            return p.function_command(|p, args, _| match args {
                [a] if p.uses_function_variable(*a) => Ok(Some(Node::Inverse(*a))),
                _ => Err("NInvert needs a function".into()),
            });
        }
        "RootList" => {
            let args = p.arguments()?;
            let list = if let [a] = args.as_slice() {
                if matches!(p.checked_type(*a)?, Ty::List(_)) {
                    *a
                } else {
                    let first = p.push(Node::Item(*a));
                    p.push(Node::List(first, 1))
                }
            } else {
                let first = p.nodes.len() as u32;
                for a in &args {
                    p.push(Node::Item(*a));
                }
                p.push(Node::List(first, args.len() as u32))
            };
            Node::RootList(list)
        }
        "Iteration" | "IterationList" => parse_iteration(p, name == "IterationList")?,
        _ => return Ok(None),
    };
    Ok(Some(p.push(node)))
}

fn point_argument(p: &Parser<'_>, node: u32) -> bool {
    match p.node(node) {
        Node::Obj(id) => p.c.kind(id) == Kind::Point,
        Node::Vector(_) => false,
        _ => p.is_point_or_vector(node),
    }
}

pub(super) fn normalize<const M: bool>(ctx: &Ctx<'_, M>, l: u32) -> Option<(u32, u32)> {
    let lists = ctx.lists?;
    let (start, len) = ctx.list(l)?;
    if len == 0 || !lists.charge(2 * u64::from(len)) {
        return None;
    }
    let read = |i| lists.read(ctx.c, start + i);
    let kind = read(0).kind;
    let points = kind == Kind::Point;
    if !points && !kind.is_number() {
        return None;
    }
    let coordinate = |slot: Slot| {
        if points {
            match slot_val(&slot) {
                Val::V(x, y) => [x, y],
                _ => [f64::NAN; 2],
            }
        } else {
            [
                slot.numeric_cell()
                    .or_else(|| number_of(slot))
                    .unwrap_or(f64::NAN),
                0.0,
            ]
        }
    };
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for i in 0..len {
        let slot = read(i);
        if points && slot.kind != Kind::Point {
            return None;
        }
        for (axis, value) in coordinate(slot).into_iter().enumerate() {
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    if !points && !read(len - 1).kind.is_number() {
        return Some((lists.len(), 0));
    }
    let scale = std::array::from_fn::<_, 2, _>(|i| {
        if max[i] == min[i] {
            max[i]
        } else {
            max[i] - min[i]
        }
    });
    let out = lists.reserve(len)?;
    for i in 0..len {
        let xy = coordinate(read(i));
        let scaled = |axis: usize| {
            if scale[axis] == 0.0 {
                0.0
            } else {
                (xy[axis] - min[axis]) / scale[axis]
            }
        };
        let slot = if points {
            Slot::coords(Kind::Point, scaled(0), scaled(1))
        } else {
            Slot::number(scaled(0))
        };
        lists.set(out + i, slot);
    }
    Some((out, len))
}

fn parse_iteration(p: &mut Parser<'_>, collect: bool) -> Result<Node, String> {
    p.expect(&Tok::LParen, "(")?;
    let start = p.pos;
    let mut depth = 0;
    let mut commas = Vec::new();
    for (index, token) in p.toks.iter().enumerate().skip(start) {
        match token.tok {
            Tok::LParen | Tok::LBrace => depth += 1,
            Tok::RParen if depth == 0 => break,
            Tok::RParen | Tok::RBrace => depth -= 1,
            Tok::Comma if depth == 0 => commas.push(index),
            _ => {}
        }
    }
    let [first, rest @ .., _] = commas.as_slice() else {
        return Err("Iteration needs a function, initial values and count".into());
    };
    let first = *first;
    let initial_start = rest.last().copied().unwrap_or(first) + 1;
    p.pos = initial_start;
    let initial = p.expression()?;
    p.expect(&Tok::Comma, ",")?;
    let count = p.expression()?;
    p.expect(&Tok::RParen, ")")?;
    let end = p.pos;
    let initial_type = p.checked_type(initial)?;
    let previous_var = p.uses_var;
    let previous_equation = p.equation;
    let local_start = p.locals.len();
    if rest.len() > super::super::parse::MAX_DEPTH as usize {
        return Err("too many iteration variables".into());
    }
    let mode = if !rest.is_empty() {
        let Ty::List(elem) = initial_type else {
            return Err("local iteration needs a list of initial values".into());
        };
        let mut last = first;
        for &separator in rest {
            let local = match &p.toks[last + 1].tok {
                Tok::Name { text, .. } if separator == last + 2 => text,
                _ => return Err("expected an iteration variable".into()),
            };
            if p.locals[local_start..]
                .iter()
                .any(|(name, _)| name == local)
            {
                return Err("duplicate iteration variable".into());
            }
            p.locals.push((local.to_string(), elem));
            last = separator;
        }
        IterationMode::Local {
            first: local_start as u32,
            count: rest.len() as u32,
        }
    } else if matches!(initial_type, Ty::List(_)) {
        p.equation = true;
        IterationMode::Indexed
    } else {
        IterationMode::Unary
    };
    p.pos = start;
    let formula = if rest.is_empty() {
        p.iteration_argument(matches!(mode, IterationMode::Indexed))
    } else {
        p.argument(false)
    };
    p.locals.truncate(local_start);
    p.uses_var = previous_var;
    p.equation = previous_equation;
    if p.pos != first {
        return Err("invalid iteration formula".into());
    }
    p.pos = end;
    Ok(Node::Iteration {
        mode,
        collect,
        formula: formula?,
        initial,
        count,
    })
}

pub(in crate::expr) fn iteration_type(
    mode: IterationMode,
    collect: bool,
    formula: Ty,
    initial: Ty,
    count: Ty,
) -> Result<Ty, String> {
    let ty = match (mode, formula, initial) {
        (IterationMode::Unary, Ty::Num(_), Ty::Num(_))
        | (IterationMode::Indexed, Ty::Num(_), Ty::List(Elem::Num | Elem::Empty)) => Ty::Num(0),
        (
            IterationMode::Local { .. },
            formula @ (Ty::Num(_) | Ty::Vec | Ty::Bool),
            Ty::List(elem),
        ) if elem == elem_of(formula) || elem == Elem::Empty => ty_of(elem_of(formula))?,
        _ => return Err("invalid iteration formula or starting values".into()),
    };
    if !matches!(count, Ty::Num(_)) {
        return Err("iteration count must be a number".into());
    }
    Ok(if collect { Ty::List(elem_of(ty)) } else { ty })
}

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn function_asymptotes(&self, source: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let values = match self.node(source) {
            Node::Apply(f, _) => crate::algo::analysis::asymptotes(self.c, f)?,
            Node::Obj(curve) => {
                crate::algo::implicit::asymptotes(self.c, curve, &|n| lists.charge(n))?
            }
            _ => return None,
        };
        let len = values.len() as u32;
        let start = lists.reserve(len)?;
        for (i, line) in values.into_iter().enumerate() {
            lists.set(start + i as u32, Slot::line(line));
        }
        Some((start, len))
    }

    pub(crate) fn root_list(&self, node: u32) -> Option<(u32, u32)> {
        let lists = self.lists?;
        let (source, len) = self.list(node)?;
        if len == 0 || !lists.charge(u64::from(len) * 2) {
            return None;
        }
        let numeric = (0..len)
            .filter(|i| lists.read(self.c, source + i).kind == Kind::Number)
            .count() as u32;
        let target = lists.reserve(numeric)?;
        let mut offset = 0;
        for i in 0..len {
            let slot = lists.read(self.c, source + i);
            if slot.kind == Kind::Number {
                let x = match slot.value {
                    Value::Number(x) => x,
                    _ => f64::NAN,
                };
                lists.set(target + offset, Slot::coords(Kind::Point, x, 0.0));
                offset += 1;
            }
        }
        Some((target, numeric))
    }

    pub(crate) fn iteration_list(
        &self,
        mode: IterationMode,
        formula: u32,
        initial: u32,
        count: u32,
    ) -> Option<(u32, u32)> {
        let count = self
            .iteration_count(count)
            .filter(|&n| u64::from(n) < MAX_ELEMENTS)?;
        let lists = self.lists?;
        let start = lists.reserve(count.checked_add(1)?)?;
        self.iterate(mode, formula, initial, count, Some(start))?;
        Some((start, count + 1))
    }

    fn iteration_count(&self, node: u32) -> Option<u32> {
        let n = self.full_number(node);
        if !n.is_finite() {
            return None;
        }
        let n = crate::expr::math::java_round(n) as i32;
        u32::try_from(n).ok()
    }

    pub(crate) fn iteration_value(
        &self,
        mode: IterationMode,
        formula: u32,
        initial: u32,
        count: u32,
    ) -> Option<Val> {
        self.iterate(mode, formula, initial, self.iteration_count(count)?, None)
    }

    fn iterate(
        &self,
        mode: IterationMode,
        formula: u32,
        initial: u32,
        steps: u32,
        output: Option<u32>,
    ) -> Option<Val> {
        let lists = self.lists?;
        let write = |i, val| {
            if let Some(start) = output {
                lists.set(
                    start + i,
                    match val {
                        Val::N(x) => Slot::number(x),
                        Val::V(x, y) => Slot::coords(Kind::Point, x, y),
                        Val::B(x, true) => Slot::boolean(x),
                        Val::B(_, false) => Slot::undefined(Kind::Boolean),
                    },
                );
            }
        };
        if let IterationMode::Local { first, count } = mode {
            let (start, len) = self.list(initial)?;
            if len < count || !lists.charge(u64::from(len.min(steps + 1))) {
                return None;
            }
            let seed = |i| slot_val(&lists.read(self.c, start + i));
            for i in 0..len.min(steps + 1) {
                write(i, seed(i));
            }
            if steps < len {
                return Some(seed(steps));
            }
            let mut history: Vec<_> = (len - count..len).map(seed).collect();
            let mut value = *history.last()?;
            for i in len..=steps {
                if !lists.charge(u64::from(count)) {
                    return None;
                }
                value = local_formula(self, formula, first, &history);
                history.rotate_left(1);
                *history.last_mut()? = value;
                write(i, value);
            }
            return Some(value);
        }
        let (mut value, offset) = if matches!(mode, IterationMode::Indexed) {
            let (start, 2) = self.list(initial)? else {
                return None;
            };
            let a = crate::expr::eval::number_of(lists.read(self.c, start))?;
            let b = crate::expr::eval::number_of(lists.read(self.c, start + 1))?;
            if output.is_some() {
                let offset = f64::from(crate::expr::math::java_round(a) as i32);
                if (offset - a).abs() >= crate::tol::STANDARD_PRECISION {
                    return None;
                }
                (b, offset)
            } else {
                (a, crate::expr::math::java_round(b) as f64)
            }
        } else {
            (self.full_number(initial), 0.0)
        };
        if !value.is_finite() || !offset.is_finite() {
            return None;
        }
        write(0, Val::N(value));
        for i in 0..steps {
            if !lists.charge(1) {
                return None;
            }
            let env = if matches!(mode, IterationMode::Indexed) {
                Env::Pair(offset + f64::from(i), value)
            } else {
                Env::Value(value)
            };
            value = Ctx { env: &env, ..*self }.fast(formula);
            write(i + 1, Val::N(value));
        }
        Some(Val::N(value))
    }
}

fn local_formula<const M: bool>(ctx: &Ctx<'_, M>, node: u32, depth: u32, values: &[Val]) -> Val {
    if let Some((&value, rest)) = values.split_first() {
        let local = Local {
            depth,
            value,
            run: None,
            vector: false,
            up: ctx.locals,
        };
        local_formula(
            &Ctx {
                locals: Some(&local),
                ..*ctx
            },
            node,
            depth + 1,
            rest,
        )
    } else {
        ctx.full(node).val
    }
}

#[cfg(test)]
mod tests {
    use crate::{script, Value};

    #[test]
    fn oversized_iteration_is_undefined_before_allocating_output() {
        let c = script::load("n = 1000000000\nr = Iteration(x+1,1,n)\nl = IterationList(x+1,1,n)")
            .unwrap();
        for label in ["r", "l"] {
            assert_eq!(c.value(c.id(label).unwrap()), Value::Undefined);
        }
    }
}
