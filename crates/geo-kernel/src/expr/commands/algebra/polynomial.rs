// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/AlgoPolynomialDivision.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::eval::{Ctx, Env};
use crate::expr::{self, Entry, IntersectionWork, Lit, Node};
use crate::kernel::{Construction, Id, Kind};

pub(in crate::expr) fn divide(
    c: &Construction,
    entry: &Entry,
    remainder: bool,
    a: u32,
    b: u32,
) -> Option<Vec<f64>> {
    let work = IntersectionWork::new();
    let ctx = Ctx::<true> {
        meter: Some(&work.0),
        ..Ctx::new(c, entry, &Env::None)
    };
    let mut dividend = work.polynomial_at(&ctx, a)?;
    let divisor = work.polynomial_at(&ctx, b)?;
    if dividend.len() < divisor.len() {
        return Some(if remainder { dividend } else { vec![0.0] });
    }
    let &leading = divisor.last()?;
    let steps = dividend.len() - divisor.len() + 1;
    if !work.charge((steps * divisor.len()) as u64) {
        return None;
    }
    for i in (0..steps).rev() {
        let end = i + divisor.len() - 1;
        dividend[end] /= leading;
        let factor = dividend[end];
        if factor != 0.0 {
            for (value, &coefficient) in dividend[i..end].iter_mut().zip(&divisor) {
                *value += -coefficient * factor;
            }
        }
    }
    let result = if remainder {
        dividend.truncate(divisor.len() - 1);
        dividend
    } else {
        dividend.split_off(divisor.len() - 1)
    };
    Some(result)
}

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn polynomial_value(&self) -> f64 {
        let Some(coefficients) = self.e.polynomial() else {
            return f64::NAN;
        };
        if self.stop(coefficients.len() as u64) {
            return f64::NAN;
        }
        let x = self.variable();
        coefficients
            .iter()
            .rev()
            .copied()
            .reduce(|sum, coefficient| sum * x + coefficient)
            .unwrap_or(0.0)
    }
}

/// The two independently sampleable outputs share the normal expression
/// and list storage. Roll back the helper functions if adding the list fails.
pub(crate) fn division_list(
    c: &mut Construction,
    label: &str,
    input: &str,
) -> Option<Result<Id, String>> {
    if !input.starts_with("Division") {
        return None;
    }
    let ("Division", args) = crate::script::split_call(input)? else {
        return None;
    };
    let &[a, b] = args.as_slice() else {
        return None;
    };
    let div = expr::parse::parse(c, &format!("Div({a},{b})")).ok()?;
    let Node::PolynomialPart(false, first, second) = div.nodes[div.root as usize] else {
        return None;
    };
    let start = c.checkpoint();
    let result = (|| {
        let mut rem = div.clone();
        rem.nodes[rem.root as usize] = Node::PolynomialPart(true, first, second);
        let quotient_label = format!("\0quotient{}", c.len());
        let quotient = expr::add(c, &quotient_label, div, Kind::Function)?;
        let remainder_label = format!("\0remainder{}", c.len());
        let remainder = expr::add(c, &remainder_label, rem, Kind::Function)?;
        let nodes = [
            Node::FunctionObject(quotient),
            Node::FunctionObject(remainder),
            Node::Item(0),
            Node::Item(1),
            Node::List(2, 2),
        ];
        let literals = vec![Lit::default(); nodes.len()];
        let parsed = expr::reparse(c, Kind::List, &nodes, &literals, 4)
            .ok_or("cannot build the polynomial division list")?;
        expr::add(c, label, parsed, Kind::List)
    })();
    if result.is_err() {
        c.discard_since(start);
    }
    Some(result)
}
