// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntervalMidpoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoIntervalUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::algo::conic::ConicDef;
use crate::expr::{self, parse, Bin, IfMode, Node, Ty, Un, NONE};
use crate::kernel::{Construction, Id, Kind};

impl ConicDef {
    /// `CmdMidpoint` accepts a one-variable boolean function. The script
    /// can supply it inline in either `Center` or `Midpoint`.
    pub(crate) fn exec_interval(
        c: &mut Construction,
        label: &str,
        rhs: &str,
    ) -> Option<Result<Option<Id>, String>> {
        let (command, arg) = rhs.split_once('(')?;
        if !matches!(command.trim(), "Center" | "Midpoint") {
            return None;
        }
        let mut parsed = parse::parse(c, arg.strip_suffix(')')?).ok()?;
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
            Some(bounds) => bounds.midpoint(&mut parsed),
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
