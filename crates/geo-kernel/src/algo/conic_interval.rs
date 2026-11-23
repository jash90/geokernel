// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdMidpoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntervalMidpoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoIntervalUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::algo::conic::ConicDef;
use crate::expr::{self, parse, Bin, IfMode, Node, Ty, Un, NONE};
use crate::kernel::{Construction, Id, Kind, Value};

impl ConicDef {
    pub(crate) fn named_interval_midpoint(c: &Construction, function: Id) -> Option<Value> {
        let crate::kernel::Def::Expr(def) = c.def(function) else {
            return None;
        };
        if !c.value(function).is_defined() {
            return None;
        }
        let entry = expr::entry(c, def);
        let bounds = Bounds::read(&entry.nodes, entry.root)?;
        let meter = expr::eval::Lists::new(Vec::new());
        let ctx = expr::eval::Ctx::<true> {
            meter: Some(&meter),
            ..expr::eval::Ctx::new(c, entry, &expr::eval::Env::None)
        };
        let bound = |nodes: Vec<u32>, initial: f64, combine: fn(f64, f64) -> f64| {
            nodes.into_iter().try_fold(initial, |acc, node| {
                let value = ctx.full_number(node);
                (!value.is_nan()).then(|| combine(acc, value))
            })
        };
        let low = bound(bounds.lower, f64::NEG_INFINITY, f64::max)?;
        let high = bound(bounds.upper, f64::INFINITY, f64::min)?;
        let center = (low + high) / 2.0;
        (low <= high && !center.is_nan() && !meter.over()).then_some(Value::Number(center))
    }

    /// `CmdMidpoint` accepts a one-variable boolean function. The script
    /// can supply it inline in either `Center` or `Midpoint`.
    pub(crate) fn exec_interval(
        c: &mut Construction,
        label: &str,
        rhs: &str,
    ) -> Option<Result<Option<Id>, String>> {
        let (command, arg) = rhs.split_once('(')?;
        if !matches!(command.trim(), "Center" | "Midpoint" | "Min" | "Max") {
            return None;
        }
        let arg = arg.strip_suffix(')')?.trim();
        // A labelled interval stores the same expression tree as an inline one.
        let mut parsed = if let Some(id) = c
            .id(arg)
            .filter(|_| matches!(command.trim(), "Min" | "Max"))
        {
            let crate::kernel::Def::Expr(def) = c.def(id) else {
                return None;
            };
            if c.kind(id) != Kind::Function {
                return None;
            }
            let entry = expr::entry(c, def);
            expr::reparse(c, Kind::Function, &entry.nodes, &entry.lits, entry.root)?
        } else {
            parse::parse(c, arg).ok()?
        };
        if !parsed.uses_var || parsed.tys[parsed.root as usize] != Ty::Bool {
            return None;
        }
        let bounds = Bounds::read(&parsed.nodes, parsed.root);
        // The result is a number. The inequality's free variable is only
        // used to find the bounds and has no value in the resulting tree.
        for node in &mut parsed.nodes {
            if matches!(node, Node::Var) {
                *node = Node::Num(f64::NAN);
            }
        }
        let root = match bounds {
            Some(bounds) => match command.trim() {
                "Min" => bounds.endpoint(&mut parsed, true),
                "Max" => bounds.endpoint(&mut parsed, false),
                _ => bounds.midpoint(&mut parsed),
            },
            None => push(&mut parsed, Node::Num(f64::NAN)),
        };
        let result = expr::reparse(c, Kind::Number, &parsed.nodes, &parsed.lits, root)
            .ok_or_else(|| "the interval midpoint expression is too complex".to_string())
            .and_then(|parsed| expr::add(c, label, parsed, Kind::Number));
        Some(result.map(Some))
    }
}

#[derive(Default)]
struct Bounds {
    lower: Vec<u32>,
    upper: Vec<u32>,
}

impl Bounds {
    /// GeoIntervalUtil walks conjunctions of comparisons between the
    /// function variable itself and an expression independent of it.
    fn read(nodes: &[Node], root: u32) -> Option<Self> {
        let mut uses_var = Vec::with_capacity(nodes.len());
        for node in nodes {
            uses_var.push(
                matches!(node, Node::Var)
                    || expr::children(node).any(|child| uses_var[child as usize]),
            );
        }
        let mut bounds = Self::default();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let (small, large) = match nodes[id as usize] {
                Node::Bin(Bin::And, left, right) => {
                    pending.extend([right, left]);
                    continue;
                }
                Node::Bin(Bin::Less | Bin::LessEqual, a, b)
                | Node::Bin(Bin::Greater | Bin::GreaterEqual, b, a) => (a, b),
                _ => return None,
            };
            match (nodes[small as usize], nodes[large as usize]) {
                (Node::Var, _) if !uses_var[large as usize] => bounds.upper.push(large),
                (_, Node::Var) if !uses_var[small as usize] => bounds.lower.push(small),
                _ => return None,
            }
        }
        Some(bounds)
    }

    fn endpoint(self, parsed: &mut parse::Parsed, minimum: bool) -> u32 {
        let low = combine(parsed, self.lower, Bin::Max, f64::NEG_INFINITY);
        let high = combine(parsed, self.upper, Bin::Min, f64::INFINITY);
        let width = push(parsed, Node::Bin(Bin::Sub, high, low));
        let root = push(parsed, Node::Un(Un::Sqrt, width));
        let zero = push(parsed, Node::Num(0.0));
        let valid = push(parsed, Node::Bin(Bin::GreaterEqual, root, zero));
        push(
            parsed,
            Node::If(IfMode::Algo, valid, if minimum { low } else { high }, NONE),
        )
    }

    fn midpoint(self, parsed: &mut parse::Parsed) -> u32 {
        let low = combine(parsed, self.lower, Bin::Max, f64::NEG_INFINITY);
        let high = combine(parsed, self.upper, Bin::Min, f64::INFINITY);
        let sum = push(parsed, Node::Bin(Bin::Add, low, high));
        let two = push(parsed, Node::Num(2.0));
        let midpoint = push(parsed, Node::Bin(Bin::Div, sum, two));

        // The square root is NaN for every negative width, even one
        // smaller than the expression engine's comparison tolerance.
        let width = push(parsed, Node::Bin(Bin::Sub, high, low));
        let root = push(parsed, Node::Un(Un::Sqrt, width));
        let zero = push(parsed, Node::Num(0.0));
        let valid = push(parsed, Node::Bin(Bin::GreaterEqual, root, zero));
        push(parsed, Node::If(IfMode::Algo, valid, midpoint, NONE))
    }
}

fn combine(parsed: &mut parse::Parsed, bounds: Vec<u32>, op: Bin, empty: f64) -> u32 {
    bounds
        .into_iter()
        .reduce(|a, b| push(parsed, Node::Bin(op, a, b)))
        .unwrap_or_else(|| push(parsed, Node::Num(empty)))
}

fn push(parsed: &mut parse::Parsed, node: Node) -> u32 {
    parsed.nodes.push(node);
    parsed.lits.push(Default::default());
    (parsed.nodes.len() - 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::load;

    #[test]
    fn center_reads_the_current_bounds_of_a_named_boolean_function() {
        let mut c = load("a = -4\nb = 2\nf(x) = a < x < b\nm = Center(f)").unwrap();
        let b = c.id("b").unwrap();
        let m = c.id("m").unwrap();
        assert_eq!(c.value(m), Value::Number(-1.0));
        c.set_number(b, -6.0).unwrap();
        assert_eq!(c.value(m), Value::Undefined);
        c.set_number(b, 4.0).unwrap();
        assert_eq!(c.value(m), Value::Number(0.0));
    }
}
