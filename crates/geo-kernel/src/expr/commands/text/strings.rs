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
        Sum => {
            // `Sum` of texts concatenates them; one list argument
            // concatenates its elements (`TextFold`)
            let mut parts: Vec<String> = Vec::new();
            if args.len() == 1 && ctx.list(a).is_some() {
                let lists = ctx.lists?;
                let (start, len) = ctx.list(a)?;
                for index in start..start + len {
                    if ctx.stop(1) {
                        return None;
                    }
                    parts.push(lists.read(ctx.c, index).text_value()?.to_owned());
                }
            } else {
                for &i in args {
                    parts.push(ctx.text(i)?.into_owned());
                }
            }
            let total: usize = parts.iter().map(String::len).sum();
            ctx.bounded_text(total)?;
            Some(parts.concat())
        }
        First | Last => {
            // `AlgoFirstString` and `AlgoLastString`: `(int)` of `n`, one
            // without it; an empty text is undefined, a length of 0 is ""
            let count = match b {
                None => 1,
                Some(n) => number(n) as i32,
            };
            let text = ctx.text(a)?;
            ctx.bounded_text(text.len())?;
            let units: Vec<u16> = text.encode_utf16().collect();
            if count < 0 || count as usize > units.len() || units.is_empty() {
                return None;
            }
            let cut = if op == TextOp::First {
                0
            } else {
                units.len() - count as usize
            };
            Some(from_utf16(&units[cut..cut + count as usize]))
        }
        TextElement => {
            // `AlgoTextElement`: `(int)` of `n` picks one UTF-16 unit
            let index = number(b?) as i32;
            let text = ctx.text(a)?;
            ctx.bounded_text(text.len())?;
            let units: Vec<u16> = text.encode_utf16().collect();
            if index < 1 || index as usize > units.len() {
                return None;
            }
            Some(from_utf16(&[units[index as usize - 1]]))
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

/// The precomposed Latin characters that fold to a bare letter when their
/// accents are stripped (`Normalizer.Form.NFD` of `StringUtil.removeAccents`,
/// Latin-1 Supplement and Latin Extended-A/B up to U+024F; the marks
/// themselves live in the stripped range below).
const DECOMPOSE: &[(u32, char)] = &[
    (0x00C0, '\u{0041}'),
    (0x00C1, '\u{0041}'),
    (0x00C2, '\u{0041}'),
    (0x00C3, '\u{0041}'),
    (0x00C4, '\u{0041}'),
    (0x00C5, '\u{0041}'),
    (0x00C7, '\u{0043}'),
    (0x00C8, '\u{0045}'),
    (0x00C9, '\u{0045}'),
    (0x00CA, '\u{0045}'),
    (0x00CB, '\u{0045}'),
    (0x00CC, '\u{0049}'),
    (0x00CD, '\u{0049}'),
    (0x00CE, '\u{0049}'),
    (0x00CF, '\u{0049}'),
    (0x00D1, '\u{004E}'),
    (0x00D2, '\u{004F}'),
    (0x00D3, '\u{004F}'),
    (0x00D4, '\u{004F}'),
    (0x00D5, '\u{004F}'),
    (0x00D6, '\u{004F}'),
    (0x00D9, '\u{0055}'),
    (0x00DA, '\u{0055}'),
    (0x00DB, '\u{0055}'),
    (0x00DC, '\u{0055}'),
    (0x00DD, '\u{0059}'),
    (0x00E0, '\u{0061}'),
    (0x00E1, '\u{0061}'),
    (0x00E2, '\u{0061}'),
    (0x00E3, '\u{0061}'),
    (0x00E4, '\u{0061}'),
    (0x00E5, '\u{0061}'),
    (0x00E7, '\u{0063}'),
    (0x00E8, '\u{0065}'),
    (0x00E9, '\u{0065}'),
    (0x00EA, '\u{0065}'),
    (0x00EB, '\u{0065}'),
    (0x00EC, '\u{0069}'),
    (0x00ED, '\u{0069}'),
    (0x00EE, '\u{0069}'),
    (0x00EF, '\u{0069}'),
    (0x00F1, '\u{006E}'),
    (0x00F2, '\u{006F}'),
    (0x00F3, '\u{006F}'),
    (0x00F4, '\u{006F}'),
    (0x00F5, '\u{006F}'),
    (0x00F6, '\u{006F}'),
    (0x00F9, '\u{0075}'),
    (0x00FA, '\u{0075}'),
    (0x00FB, '\u{0075}'),
    (0x00FC, '\u{0075}'),
    (0x00FD, '\u{0079}'),
    (0x00FF, '\u{0079}'),
    (0x0100, '\u{0041}'),
    (0x0101, '\u{0061}'),
    (0x0102, '\u{0041}'),
    (0x0103, '\u{0061}'),
    (0x0104, '\u{0041}'),
    (0x0105, '\u{0061}'),
    (0x0106, '\u{0043}'),
    (0x0107, '\u{0063}'),
    (0x0108, '\u{0043}'),
    (0x0109, '\u{0063}'),
    (0x010A, '\u{0043}'),
    (0x010B, '\u{0063}'),
    (0x010C, '\u{0043}'),
    (0x010D, '\u{0063}'),
    (0x010E, '\u{0044}'),
    (0x010F, '\u{0064}'),
    (0x0112, '\u{0045}'),
    (0x0113, '\u{0065}'),
    (0x0114, '\u{0045}'),
    (0x0115, '\u{0065}'),
    (0x0116, '\u{0045}'),
    (0x0117, '\u{0065}'),
    (0x0118, '\u{0045}'),
    (0x0119, '\u{0065}'),
    (0x011A, '\u{0045}'),
    (0x011B, '\u{0065}'),
    (0x011C, '\u{0047}'),
    (0x011D, '\u{0067}'),
    (0x011E, '\u{0047}'),
    (0x011F, '\u{0067}'),
    (0x0120, '\u{0047}'),
    (0x0121, '\u{0067}'),
    (0x0122, '\u{0047}'),
    (0x0123, '\u{0067}'),
    (0x0124, '\u{0048}'),
    (0x0125, '\u{0068}'),
    (0x0128, '\u{0049}'),
    (0x0129, '\u{0069}'),
    (0x012A, '\u{0049}'),
    (0x012B, '\u{0069}'),
    (0x012C, '\u{0049}'),
    (0x012D, '\u{0069}'),
    (0x012E, '\u{0049}'),
    (0x012F, '\u{0069}'),
    (0x0130, '\u{0049}'),
    (0x0134, '\u{004A}'),
    (0x0135, '\u{006A}'),
    (0x0136, '\u{004B}'),
    (0x0137, '\u{006B}'),
    (0x0139, '\u{004C}'),
    (0x013A, '\u{006C}'),
    (0x013B, '\u{004C}'),
    (0x013C, '\u{006C}'),
    (0x013D, '\u{004C}'),
    (0x013E, '\u{006C}'),
    (0x0143, '\u{004E}'),
    (0x0144, '\u{006E}'),
    (0x0145, '\u{004E}'),
    (0x0146, '\u{006E}'),
    (0x0147, '\u{004E}'),
    (0x0148, '\u{006E}'),
    (0x014C, '\u{004F}'),
    (0x014D, '\u{006F}'),
    (0x014E, '\u{004F}'),
    (0x014F, '\u{006F}'),
    (0x0150, '\u{004F}'),
    (0x0151, '\u{006F}'),
    (0x0154, '\u{0052}'),
    (0x0155, '\u{0072}'),
    (0x0156, '\u{0052}'),
    (0x0157, '\u{0072}'),
    (0x0158, '\u{0052}'),
    (0x0159, '\u{0072}'),
    (0x015A, '\u{0053}'),
    (0x015B, '\u{0073}'),
    (0x015C, '\u{0053}'),
    (0x015D, '\u{0073}'),
    (0x015E, '\u{0053}'),
    (0x015F, '\u{0073}'),
    (0x0160, '\u{0053}'),
    (0x0161, '\u{0073}'),
    (0x0162, '\u{0054}'),
    (0x0163, '\u{0074}'),
    (0x0164, '\u{0054}'),
    (0x0165, '\u{0074}'),
    (0x0168, '\u{0055}'),
    (0x0169, '\u{0075}'),
    (0x016A, '\u{0055}'),
    (0x016B, '\u{0075}'),
    (0x016C, '\u{0055}'),
    (0x016D, '\u{0075}'),
    (0x016E, '\u{0055}'),
    (0x016F, '\u{0075}'),
    (0x0170, '\u{0055}'),
    (0x0171, '\u{0075}'),
    (0x0172, '\u{0055}'),
    (0x0173, '\u{0075}'),
    (0x0174, '\u{0057}'),
    (0x0175, '\u{0077}'),
    (0x0176, '\u{0059}'),
    (0x0177, '\u{0079}'),
    (0x0178, '\u{0059}'),
    (0x0179, '\u{005A}'),
    (0x017A, '\u{007A}'),
    (0x017B, '\u{005A}'),
    (0x017C, '\u{007A}'),
    (0x017D, '\u{005A}'),
    (0x017E, '\u{007A}'),
    (0x01A0, '\u{004F}'),
    (0x01A1, '\u{006F}'),
    (0x01AF, '\u{0055}'),
    (0x01B0, '\u{0075}'),
    (0x01CD, '\u{0041}'),
    (0x01CE, '\u{0061}'),
    (0x01CF, '\u{0049}'),
    (0x01D0, '\u{0069}'),
    (0x01D1, '\u{004F}'),
    (0x01D2, '\u{006F}'),
    (0x01D3, '\u{0055}'),
    (0x01D4, '\u{0075}'),
    (0x01D5, '\u{0055}'),
    (0x01D6, '\u{0075}'),
    (0x01D7, '\u{0055}'),
    (0x01D8, '\u{0075}'),
    (0x01D9, '\u{0055}'),
    (0x01DA, '\u{0075}'),
    (0x01DB, '\u{0055}'),
    (0x01DC, '\u{0075}'),
    (0x01DE, '\u{0041}'),
    (0x01DF, '\u{0061}'),
    (0x01E0, '\u{0041}'),
    (0x01E1, '\u{0061}'),
    (0x01E2, '\u{00C6}'),
    (0x01E3, '\u{00E6}'),
    (0x01E6, '\u{0047}'),
    (0x01E7, '\u{0067}'),
    (0x01E8, '\u{004B}'),
    (0x01E9, '\u{006B}'),
    (0x01EA, '\u{004F}'),
    (0x01EB, '\u{006F}'),
    (0x01EC, '\u{004F}'),
    (0x01ED, '\u{006F}'),
    (0x01EE, '\u{01B7}'),
    (0x01EF, '\u{0292}'),
    (0x01F0, '\u{006A}'),
    (0x01F4, '\u{0047}'),
    (0x01F5, '\u{0067}'),
    (0x01F8, '\u{004E}'),
    (0x01F9, '\u{006E}'),
    (0x01FA, '\u{0041}'),
    (0x01FB, '\u{0061}'),
    (0x01FC, '\u{00C6}'),
    (0x01FD, '\u{00E6}'),
    (0x01FE, '\u{00D8}'),
    (0x01FF, '\u{00F8}'),
    (0x0200, '\u{0041}'),
    (0x0201, '\u{0061}'),
    (0x0202, '\u{0041}'),
    (0x0203, '\u{0061}'),
    (0x0204, '\u{0045}'),
    (0x0205, '\u{0065}'),
    (0x0206, '\u{0045}'),
    (0x0207, '\u{0065}'),
    (0x0208, '\u{0049}'),
    (0x0209, '\u{0069}'),
    (0x020A, '\u{0049}'),
    (0x020B, '\u{0069}'),
    (0x020C, '\u{004F}'),
    (0x020D, '\u{006F}'),
    (0x020E, '\u{004F}'),
    (0x020F, '\u{006F}'),
    (0x0210, '\u{0052}'),
    (0x0211, '\u{0072}'),
    (0x0212, '\u{0052}'),
    (0x0213, '\u{0072}'),
    (0x0214, '\u{0055}'),
    (0x0215, '\u{0075}'),
    (0x0216, '\u{0055}'),
    (0x0217, '\u{0075}'),
    (0x0218, '\u{0053}'),
    (0x0219, '\u{0073}'),
    (0x021A, '\u{0054}'),
    (0x021B, '\u{0074}'),
    (0x021E, '\u{0048}'),
    (0x021F, '\u{0068}'),
    (0x0226, '\u{0041}'),
    (0x0227, '\u{0061}'),
    (0x0228, '\u{0045}'),
    (0x0229, '\u{0065}'),
    (0x022A, '\u{004F}'),
    (0x022B, '\u{006F}'),
    (0x022C, '\u{004F}'),
    (0x022D, '\u{006F}'),
    (0x022E, '\u{004F}'),
    (0x022F, '\u{006F}'),
    (0x0230, '\u{004F}'),
    (0x0231, '\u{006F}'),
    (0x0232, '\u{0059}'),
    (0x0233, '\u{0079}'),
];

/// `StringUtil.removeAccents`: US lowercase, canonical decomposition, then
/// the combining marks of the Latin block stripped (`GeoText.getComparator`
/// sorts by this first).
pub(crate) fn remove_accents(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match DECOMPOSE.binary_search_by_key(&u32::from(c), |&(cp, _)| cp) {
            Ok(k) => folded.push(DECOMPOSE[k].1),
            Err(_) => folded.push(c),
        }
    }
    folded
        .chars()
        .filter(|&c| !('\u{0300}'..='\u{036F}').contains(&c))
        .collect()
}

/// Java's `String.compareTo`: UTF-16 code units, not code points.
pub(crate) fn java_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut a = a.encode_utf16();
    let mut b = b.encode_utf16();
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x != y => return x.cmp(&y),
            (Some(_), Some(_)) => {}
        }
    }
}

/// Java strings count UTF-16 units. Isolated surrogate halves have no UTF-8
/// representation and are written as '?' by the Java harness.
pub(crate) fn from_utf16(units: &[u16]) -> String {
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
