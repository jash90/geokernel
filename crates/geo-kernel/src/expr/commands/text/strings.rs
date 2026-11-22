// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLetterToUnicode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnicodeToLetter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTextToUnicode.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoUnicodeToText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTakeString.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoIndexOf.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoReplaceAll.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoSplit.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::TextOp;
use crate::expr::eval::Ctx;
use crate::expr::list::Slot;
use crate::Value;

pub(super) fn evaluate<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: TextOp,
    args: &[u32],
) -> Option<String> {
    use TextOp::*;
    let a = args[0];
    let b = args.get(1).copied();
    let c = args.get(2).copied();
    let number = |i| ctx.full_number(i);
    match op {
        UnicodeToLetter => Some(from_utf16(&[number(a) as i32 as u16])),
        UnicodeToText => {
            let lists = ctx.lists?;
            let units = match ctx.list(a) {
                Some((start, len)) => {
                    ctx.bounded_text((len as usize).saturating_mul(3))?;
                    (start..start + len)
                        .filter_map(|i| {
                            let slot = lists.read(ctx.c, i);
                            slot.kind.is_number().then_some(match slot.value {
                                Value::Number(v) => v as i32 as u16,
                                _ => 0,
                            })
                        })
                        .collect()
                }
                None => Vec::new(),
            };
            Some(from_utf16(&units))
        }
        ReplaceAll => {
            let input = ctx.text(a)?;
            let needle = ctx.text(b?)?;
            let replacement = ctx.text(c?)?;
            replace(ctx, &input, &needle, &replacement)
        }
        Take => {
            let start = number(b?);
            let end = c.map(number);
            if start.is_nan() || end.is_some_and(f64::is_nan) {
                return Some(String::new());
            }
            let text = ctx.text(a)?;
            ctx.bounded_text(text.len())?;
            let units: Vec<_> = text.encode_utf16().collect();
            let start = (start as i32).checked_sub(1)? as usize;
            let end = end.map_or(units.len() as i32, |v| v as i32) as usize;
            if end == 0 && start < units.len() && c.is_some_and(|c| number(c) == 0.0) {
                return Some(String::new());
            }
            if start >= end {
                return None;
            }
            Some(from_utf16(units.get(start..end)?))
        }
        _ => None,
    }
}

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn text_number(&self, op: TextOp, first: u32, count: u32) -> f64 {
        let get = || {
            let mut args = self.text_args(first, count);
            let a = self.text(args.next()?)?;
            self.bounded_text(a.len())?;
            match op {
                TextOp::Length => Some(a.encode_utf16().count() as f64),
                TextOp::LetterToUnicode => {
                    let mut units = a.encode_utf16();
                    let unit = units.next()?;
                    units.next().is_none().then_some(f64::from(unit))
                }
                TextOp::IndexOf => {
                    let haystack = self.text(args.next()?)?;
                    let start = args
                        .next()
                        .map_or(0, |i| (self.full_number(i) as i32).wrapping_sub(1));
                    let start = usize::try_from(start).ok()?;
                    self.bounded_text(haystack.len())?;
                    let hay: Vec<_> = haystack.encode_utf16().collect();
                    if a.is_empty() {
                        return Some((start.min(hay.len()) + 1) as f64);
                    }
                    let needle: Vec<_> = a.encode_utf16().collect();
                    for (offset, window) in hay.get(start..)?.windows(needle.len()).enumerate() {
                        if self.stop(needle.len() as u64) {
                            return None;
                        }
                        if window == needle {
                            return Some((start + offset + 1) as f64);
                        }
                    }
                    None
                }
                _ => None,
            }
        };
        get().unwrap_or(f64::NAN)
    }

    pub(crate) fn text_list(&self, op: TextOp, first: u32, count: u32) -> Option<(u32, u32)> {
        let mut args = self.text_args(first, count);
        let input = self.text(args.next()?)?;
        self.bounded_text(input.len())?;
        let lists = self.lists?;
        match op {
            TextOp::TextToUnicode => {
                let len = input.encode_utf16().count() as u32;
                let start = lists.reserve(len)?;
                for (i, unit) in input.encode_utf16().enumerate() {
                    lists.set(start + i as u32, Slot::number(f64::from(unit)));
                }
                Some((start, len))
            }
            TextOp::Split => {
                let (start, len) = self.list(args.next()?)?;
                let mut parts = vec![input.into_owned()];
                for i in start..start + len {
                    let slot = lists.read(self.c, i);
                    let delimiter = slot.text_value()?;
                    let mut next = Vec::new();
                    for part in parts {
                        self.bounded_text(part.len())?;
                        if delimiter.is_empty() {
                            next.extend(part.encode_utf16().map(|unit| from_utf16(&[unit])));
                        } else {
                            next.extend(
                                part.split(delimiter)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned),
                            );
                        }
                    }
                    parts = next;
                }
                let len = parts.len() as u32;
                let start = lists.reserve(len)?;
                for (i, part) in parts.into_iter().enumerate() {
                    lists.set(start + i as u32, Slot::text(part, None));
                }
                Some((start, len))
            }
            _ => None,
        }
    }
}

/// Java strings count UTF-16 units. Isolated surrogate halves have no UTF-8
/// representation and are written as '?' by the Java harness.
fn from_utf16(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied())
        .map(|c| c.unwrap_or('?'))
        .collect()
}

fn replace<const M: bool>(
    ctx: &Ctx<'_, M>,
    input: &str,
    needle: &str,
    replacement: &str,
) -> Option<String> {
    if !input.contains(needle) {
        return Some(input.to_owned());
    }
    // The match is quoted by GeoText. The replacement uses Java's group
    // syntax, where group 0 is the whole match and backslash quotes a char.
    let mut replacement_text = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => replacement_text.push(chars.next()?),
            '$' => {
                if chars.next()? != '0' {
                    return None;
                }
                while chars.next_if_eq(&'0').is_some() {}
                ctx.bounded_text(replacement_text.len().checked_add(needle.len())?)?;
                replacement_text.push_str(needle);
            }
            _ => replacement_text.push(c),
        }
    }
    let hits = if needle.is_empty() {
        input.encode_utf16().count() + 1
    } else {
        input.matches(needle).count()
    };
    let len = input
        .len()
        .checked_sub(hits * needle.len())?
        .checked_add(hits.checked_mul(replacement_text.len())?)?;
    ctx.bounded_text(len)?;
    if needle.is_empty() && !replacement_text.is_empty() {
        let mut result = replacement_text.clone();
        for u in input.encode_utf16() {
            result.push(char::from_u32(u32::from(u)).unwrap_or('?'));
            result.push_str(&replacement_text);
        }
        Some(result)
    } else {
        Some(input.replace(needle, &replacement_text))
    }
}
