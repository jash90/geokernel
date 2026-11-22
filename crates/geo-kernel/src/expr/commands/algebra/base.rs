// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoToBase.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoFromBase.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::eval::Ctx;
use crate::expr::math::{is_integer, java_round};
use crate::expr::{Bin, IfMode, Lit, Node, Ty};
use std::borrow::Cow;

const DIGITS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// GeoGebra derives this from the length of the formatted value of 1/9.
/// The kernel has no rounding setting, so use Classic's default "0.11".
const FRACTION_DIGITS: usize = 2;

impl<const M: bool> Ctx<'_, M> {
    pub(crate) fn text(&self, i: u32) -> Option<Cow<'_, str>> {
        if self.stop(1) {
            return None;
        }
        match self.node(i) {
            Node::Relation { objects, count, .. } => {
                let ids = &objects[..usize::from(count)];
                let names = ids
                    .iter()
                    .map(|&id| self.c.label(id))
                    .filter(|name| !name.starts_with('\0'))
                    .collect::<Vec<_>>()
                    .join(", ");
                let reports =
                    crate::algo::predicate::relation::numerical(self.c, ids, |n| self.stop(n));
                let message = reports
                    .iter()
                    .map(|r| r.message)
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(Cow::Owned(if names.is_empty() {
                    message
                } else {
                    format!("{names}: {message}")
                }))
            }
            Node::Text => match &self.e.lits[i as usize] {
                Lit::Text(text) => Some(Cow::Borrowed(text)),
                _ => None,
            },
            Node::Obj(id) => self.c.text(id).map(Cow::Borrowed),
            Node::Bin(Bin::Add, a, b) => self.concatenate(a, b).map(Cow::Owned),
            Node::TextCmd(op, first, count) => self.text_command(op, first, count).map(Cow::Owned),
            Node::ListOp(op, list, index) => self
                .pick(op, list, index)?
                .text_value()
                .map(|s| Cow::Owned(s.to_owned())),
            Node::Base {
                to: true,
                value,
                radix,
            } => {
                let base = read_radix(self.full_number(radix))?;
                let value = self.full_number(value);
                if value.is_nan() {
                    return None;
                }
                let scale = f64::from(base.pow(FRACTION_DIGITS as u32));
                let rounded = if value + scale.recip() > value.ceil() {
                    value.ceil()
                } else {
                    value
                };
                let mut text = String::new();
                integer_text(rounded as i64, base, 1, &mut text, |n| self.stop(n))?;
                if !is_integer(rounded) {
                    text.push('.');
                    let fractional = java_round(scale * (value - value.floor()));
                    integer_text(fractional, base, FRACTION_DIGITS, &mut text, |n| {
                        self.stop(n)
                    })?;
                }
                Some(Cow::Owned(text))
            }
            Node::If(mode, _, first, _) => {
                let chosen = self.if_alternative(i)?;
                if mode == IfMode::Algo
                    && matches!(self.e.tys[chosen as usize], Ty::Num(_))
                    && !self.full_number(chosen).is_nan()
                {
                    // Copying a number into a text leaves the previous text.
                    if i == self.e.root {
                        if let Some(text) = self.e.text() {
                            return Some(Cow::Borrowed(text));
                        }
                    }
                    return self.text(first);
                }
                self.text(chosen)
            }
            Node::Item(a) => self.text(a),
            // the text element a `Zip` bound to its variable
            Node::Local(depth, _) => self.local(depth)?.text.map(Cow::Borrowed),
            _ => None,
        }
    }

    pub(crate) fn read_base_number(&self, value: u32, radix: u32) -> f64 {
        let calculate = || {
            let radix = read_radix(self.full_number(radix))?;
            let text = self.text(value)?;
            // GeoGebra lowercases with the US locale and operates on UTF-16
            // code units. Its integer and fractional scans use the same
            // character offset, including punctuation in higher bases.
            if self.stop(text.len() as u64) {
                return None;
            }
            let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
            let digit = |code: u16| {
                let mut digit = i32::from(code) - 0x30;
                if digit > 9 {
                    digit -= 0x30 - 9;
                }
                (0..radix as i32)
                    .contains(&digit)
                    .then_some(f64::from(digit))
            };
            let base = f64::from(radix);
            let mut result = 0.0;
            for code in whole.to_lowercase().encode_utf16() {
                result = result * base + digit(code)?;
            }
            let mut factor = 1.0;
            for code in fraction.to_lowercase().encode_utf16() {
                factor /= base;
                result += factor * digit(code)?;
            }
            Some(result)
        };
        calculate().unwrap_or(f64::NAN)
    }
}

fn read_radix(value: f64) -> Option<u32> {
    let radix = value as u32;
    (2..=36).contains(&radix).then_some(radix)
}

fn integer_text(
    value: i64,
    radix: u32,
    pad: usize,
    out: &mut String,
    stop: impl Fn(u64) -> bool,
) -> Option<()> {
    let mut value_abs = value.unsigned_abs();
    let mut buffer = [b'0'; 64];
    let mut used = 0;
    let radix = u64::from(radix);
    loop {
        if stop(1) {
            return None;
        }
        buffer[used] = DIGITS[(value_abs % radix) as usize];
        value_abs /= radix;
        used += 1;
        if value_abs == 0 {
            break;
        }
    }
    if value < 0 {
        out.push('-');
    }
    out.extend(
        buffer[..used.max(pad)]
            .iter()
            .rev()
            .map(|&digit| char::from(digit)),
    );
    Some(())
}
