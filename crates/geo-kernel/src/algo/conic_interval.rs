// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntervalMidpoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoIntervalUtil.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::kernel::{Construction, Id};

/// `CmdMidpoint` also accepts a one-variable inequality. Bind its inline
/// bounds through the ordinary expression engine, so the center updates
/// whenever a number or point in a bound changes.
pub(crate) fn exec(
    c: &mut Construction,
    label: &str,
    rhs: &str,
) -> Option<Result<Option<Id>, String>> {
    let (command, arg) = rhs.split_once('(')?;
    if command.trim() != "Center" {
        return None;
    }
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    bounds(arg.strip_suffix(')')?, &mut lower, &mut upper)?;
    let low = combined("Max", lower, "-∞");
    let high = combined("Min", upper, "∞");
    // GeoIntervalUtil rejects every negative width. Taking the square
    // root makes the test exact even though expression comparisons
    // themselves use a tolerance.
    let expr = format!("If(sqrt({high} - {low}) >= 0, ({low} + {high}) / 2)");
    Some(crate::expr::define(c, label, &expr, false).map(Some))
}

/// GeoIntervalUtil walks conjunctions and only extracts comparisons between
/// the function variable itself and a bound independent of that variable.
fn bounds<'a>(src: &'a str, lower: &mut Vec<&'a str>, upper: &mut Vec<&'a str>) -> Option<()> {
    let mut src = src.trim();
    while let Some(inner) = strip_group(src) {
        src = inner;
    }
    let mut depth = 0i32;
    let mut comparisons = Vec::new();
    for (i, ch) in src.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '∧' | '&' if depth == 0 => {
                let sep = if ch == '∧' { "∧" } else { "&&" };
                let right = src[i..].strip_prefix(sep)?;
                bounds(&src[..i], lower, upper)?;
                return bounds(right, lower, upper);
            }
            '<' | '>' | '≤' | '≥' if depth == 0 => {
                let end = i + ch.len_utf8();
                let end = end + usize::from(src[end..].starts_with('='));
                comparisons.push((i, end, matches!(ch, '<' | '≤')));
            }
            _ => {}
        }
    }
    if comparisons.is_empty() {
        return None;
    }
    let mut start = 0;
    for (index, &(i, end, less)) in comparisons.iter().enumerate() {
        let stop = comparisons.get(index + 1).map_or(src.len(), |op| op.0);
        let first = src[start..i].trim();
        let second = src[end..stop].trim();
        let (small, large) = if less {
            (first, second)
        } else {
            (second, first)
        };
        match (small, large) {
            ("x", bound) if !bound.is_empty() && bound != "x" => upper.push(bound),
            (bound, "x") if !bound.is_empty() && bound != "x" => lower.push(bound),
            _ => return None,
        }
        start = end;
    }
    Some(())
}

fn strip_group(src: &str) -> Option<&str> {
    let inner = src.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return None,
            ')' => depth -= 1,
            _ => {}
        }
    }
    (depth == 0).then_some(inner.trim())
}

fn combined(command: &str, bounds: Vec<&str>, default: &str) -> String {
    let bracketed = bounds.into_iter().map(|bound| format!("({bound})"));
    bracketed
        .reduce(|a, b| format!("{command}({a}, {b})"))
        .unwrap_or_else(|| default.to_string())
}
