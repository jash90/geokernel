// Moment formulas and command forms from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/statistics/AlgoStats2D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdOneOrTwoListsFunction.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use std::cmp::Ordering;

use super::magnitude_sample;
use crate::expr::eval::{slot_val, Ctx, Val};
use crate::expr::parse::{list_arity, Parser};
use crate::expr::{ListOp, Node, Ty, NONE};
use crate::Kind;

impl ListOp {
    pub(in crate::expr) fn takes_paired_lists(self) -> bool {
        matches!(
            self,
            Self::Covariance | Self::Pmcc | Self::Sxy | Self::SigmaXy | Self::Spearman
        )
    }

    pub(super) fn is_paired_statistic(self) -> bool {
        self.takes_paired_lists() || matches!(self, Self::Syy | Self::SigmaYy)
    }
}

pub(super) fn parse(p: &mut Parser, text: &str) -> Result<Option<u32>, String> {
    let op = match text {
        "Covariance" => ListOp::Covariance,
        "PMCC" | "CorrelationCoefficient" => ListOp::Pmcc,
        "SXY" | "Sxy" => ListOp::Sxy,
        "SigmaXY" => ListOp::SigmaXy,
        "Spearman" => ListOp::Spearman,
        "SYY" | "Syy" => ListOp::Syy,
        "SigmaYY" => ListOp::SigmaYy,
        _ => return Ok(None),
    };
    p.typed_list_command(|p, args| {
        let (first, second) = match *args {
            [(list, Some(Ty::List(_)), _)] => (list, NONE),
            [(x, Some(Ty::List(_)), _), (y, Some(Ty::List(_)), _)] if op.takes_paired_lists() => {
                (x, y)
            }
            _ if args.len() >= 2
                && args
                    .iter()
                    .all(|&(_, ty, vector)| ty == Some(Ty::Vec) && !vector) =>
            {
                let points: Vec<_> = args.iter().map(|&(root, _, _)| root).collect();
                (p.wrap_list(&points), NONE)
            }
            _ => return Err(list_arity(text, args.len())),
        };
        Ok(Some(Node::ListOp(op, first, second)))
    })
}

impl<const M: bool> Ctx<'_, M> {
    pub(super) fn paired_statistic(&self, op: ListOp, first: u32, second: u32) -> Option<f64> {
        let lists = self.lists?;
        let (start, len) = self.list(first)?;
        let second = if second == NONE {
            None
        } else {
            let (start, other_len) = self.list(second)?;
            if len == 0 || other_len != len {
                return None;
            }
            Some(start)
        };
        let pair = |k| {
            let first = lists.read(self.c, start + k);
            if let Some(second) = second {
                let second = lists.read(self.c, second + k);
                Some([magnitude_sample(first)?, magnitude_sample(second)?])
            } else if first.kind == Kind::Point {
                let Val::V(x, y) = slot_val(&first) else {
                    return None;
                };
                Some([x, y])
            } else {
                None
            }
        };
        let reads_per_row = 1 + u32::from(second.is_some());
        if op == ListOp::Spearman {
            if len < 2 {
                return None;
            }
            // Charge the sample reads, both sorts and the linear rank passes
            // before collecting any rows.
            let work_per_row = reads_per_row + 2 * (len.ilog2() + 1) + 6;
            if !lists.charge(u64::from(len) * u64::from(work_per_row)) {
                return None;
            }
            let mut rows = Vec::with_capacity(len as usize);
            for k in 0..len {
                let row = pair(k)?;
                if row[0].is_nan() || row[1].is_nan() {
                    return None;
                }
                rows.push(row);
            }
            return Some(spearman(&mut rows));
        }

        if !lists.charge(u64::from(len) * u64::from(reads_per_row)) {
            return None;
        }
        let (mut x, mut y, mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for k in 0..len {
            let [a, b] = pair(k)?;
            x += a;
            y += b;
            xx += a * a;
            yy += b * b;
            xy += a * b;
        }
        let n = f64::from(len);
        Some(match op {
            ListOp::Covariance => xy / n - (x / n) * (y / n),
            ListOp::Pmcc => (xy * n - x * y) / ((xx * n - x * x) * (yy * n - y * y)).sqrt(),
            ListOp::Sxy => xy - x * y / n,
            ListOp::SigmaXy => xy,
            ListOp::Syy => yy - y * y / n,
            ListOp::SigmaYy => yy,
            _ => unreachable!(),
        })
    }
}

/// Pearson correlation of the two rank columns. Each tie gets the mean of
/// its occupied ranks, which preserves the overall mean (n + 1) / 2.
fn spearman(rows: &mut [[f64; 2]]) -> f64 {
    let mut order: Vec<_> = (0..rows.len()).collect();
    let mean = (rows.len() as f64 + 1.0) / 2.0;
    for axis in [0, 1] {
        order.sort_unstable_by(|&a, &b| rows[a][axis].total_cmp(&rows[b][axis]));
        let mut start = 0;
        while start < order.len() {
            let value = rows[order[start]][axis];
            let mut end = start + 1;
            // Use the sort order for equality too so that -0 and +0 retain
            // distinct ranks.
            while end < order.len() && rows[order[end]][axis].total_cmp(&value) == Ordering::Equal {
                end += 1;
            }
            let rank = (start as f64 + 1.0 + end as f64) / 2.0 - mean;
            for &i in &order[start..end] {
                rows[i][axis] = rank;
            }
            start = end;
        }
    }
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for &[x, y] in rows.iter() {
        xx += x * x;
        yy += y * y;
        xy += x * y;
    }
    xy / (xx * yy).sqrt()
}

#[cfg(test)]
mod tests {
    use crate::expr::with_evaluation_budget;
    use crate::{script, Value};

    #[test]
    fn paired_statistics_charge_both_samples_and_rank_sorts() {
        let mut c = script::load("l = Sequence(1024)\nm = Reverse(l)").unwrap();
        for (command, budget, expected) in
            [("Covariance", 1536, -87381.25), ("Spearman", 20480, -1.0)]
        {
            let line = format!("a = {command}(l, m)");
            let id = with_evaluation_budget(budget, || script::exec(&mut c, &line))
                .unwrap()
                .unwrap();
            assert_eq!(c.value(id), Value::Undefined, "{command}");
            let id = script::exec(&mut c, &line).unwrap().unwrap();
            assert_eq!(c.value(id), Value::Number(expected), "{command}");
        }
    }

    #[test]
    fn paired_statistics_read_numeric_geometry() {
        let c = script::load(
            "O = (0,0)\nA = (2,0)\nB = (0,3)\np = Polygon(O,B,A)\nxy = SigmaXY({p,1},{1,2})\nrk = Spearman({p,1},{1,2})",
        )
        .unwrap();
        assert_eq!(c.number(c.id("xy").unwrap()), Some(5.0));
        assert_eq!(c.number(c.id("rk").unwrap()), Some(-1.0));
    }

    #[test]
    fn paired_statistics_require_point_arguments_or_lists() {
        let mut c =
            script::load("v = Vector((1, 2))\nL = {v}\nP = {(1,2)}\nk = 1\nM = {(1,2),v}").unwrap();
        for call in [
            "Covariance((1, 2))",
            "PMCC(1, 2)",
            "Spearman(v, (1, 2))",
            "Covariance(Element(L, 1), (0, 0))",
            "Covariance(Element({v}, 1), (0, 0))",
            "Covariance(Element(M, k+1), (0, 0))",
            "Covariance(Element(Reverse(M), 1), (0, 0))",
            "SYY({1, 2}, {3, 4})",
            "SigmaYY({1, 2}, {3, 4})",
        ] {
            assert!(
                script::exec(&mut c, &format!("a = {call}")).is_err(),
                "{call}"
            );
        }
        for call in [
            "SigmaXY({v, (1, 2)})",
            "SXY({1, 2})",
            "SYY({(1, 2), 3})",
            "PMCC({1}, {2, 3})",
            "Spearman({1}, {2, 3})",
        ] {
            let id = script::exec(&mut c, &format!("a = {call}"))
                .unwrap()
                .unwrap();
            assert_eq!(c.kind(id), crate::Kind::Number, "{call}");
            assert_eq!(c.value(id), Value::Undefined, "{call}");
        }
        let id = script::exec(&mut c, "a = SXY((1, 2), (3, 4)) + SYY({(1, 2), (3, 4)})")
            .unwrap()
            .unwrap();
        assert_eq!(c.value(id), Value::Number(4.0));
        let id = script::exec(&mut c, "a = Covariance(Element(P, 1), (0, 0))")
            .unwrap()
            .unwrap();
        assert_eq!(c.value(id), Value::Number(0.5));
    }
}
