//! Numeric asymptotes for rational functions and elementary affine arguments.
//! Ordering follows AlgoAsymptoteFunction: limits at infinity, then poles.

use super::roots::multiplicity;
use crate::algo::intersect::graph_roots;
use crate::expr::IntersectionWork;
use crate::{Construction, Id, Value};
use std::f64::consts::FRAC_PI_2;

pub(crate) fn asymptotes(c: &Construction, function: Id) -> Option<Vec<Value>> {
    if c.value(function) != Value::Function {
        return None;
    }
    let work = IntersectionWork::new();
    let mut lines = Vec::new();
    let horizontal = |height: f64| Value::Line {
        a: 0.0,
        b: 1.0,
        c: -height,
    };
    let vertical = |x: f64| Value::Line {
        a: 1.0,
        b: 0.0,
        c: -x,
    };
    if let Some((p, q)) = work.fraction(c, function) {
        let (p_end, q_end) = (*p.last()?, *q.last()?);
        if !p_end.is_finite() || !q_end.is_finite() || q_end == 0.0 {
            return None;
        }
        match p.len().checked_sub(q.len()) {
            None => lines.push(horizontal(0.0)),
            Some(0) => lines.push(horizontal(p_end / q_end)),
            Some(1) => {
                let slope = p_end / q_end;
                let q_previous = q.len().checked_sub(2).map_or(0.0, |i| q[i]);
                let intercept = (p[p.len() - 2] - slope * q_previous) / q_end;
                lines.push(Value::Line {
                    a: -slope,
                    b: 1.0,
                    c: -intercept,
                });
            }
            _ => {}
        }
        for x in graph_roots(&q, &|n| work.charge(n)) {
            if let Some((q_order, _)) = multiplicity(&q, x, &work) {
                if multiplicity(&p, x, &work).is_some_and(|(order, _)| order < q_order) {
                    lines.push(vertical(x));
                }
            }
        }
    } else if let Some((op, [b, a])) = work.analysis_transcendental(c, function) {
        match op {
            0 => lines.push(horizontal(0.0)),
            1 => {
                lines.push(horizontal(FRAC_PI_2.copysign(a)));
                lines.push(horizontal(-FRAC_PI_2.copysign(a)));
            }
            2 => lines.push(vertical(-b / a)),
            _ => unreachable!(),
        }
    } else {
        return None;
    }
    if work.exhausted() || lines.iter().any(|line| !line.is_defined()) {
        return None;
    }
    Some(lines)
}
