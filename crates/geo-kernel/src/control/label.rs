// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/CaptionBuilder.java
//   source/shared/common/src/main/java/org/geogebra/common/gui/view/algebra/GeoElementValueConverter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/util/ScientificFormatAdapter.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::{Construction, Id, Value};
use std::borrow::Cow;
use std::fmt::Write as _;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NumberRounding {
    Decimals(u32),
    SignificantFigures(u32),
}

impl Default for NumberRounding {
    fn default() -> Self {
        Self::Decimals(2)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliderLabel {
    pub mode: i32,
    pub caption: Arc<str>,
}

/// Resolves the imported codes used by default object styles.
pub fn resolve_label_mode(mode: i32, default: i32) -> i32 {
    match mode {
        0..=3 | 9 => mode,
        4 => default,
        5..=8 => mode - 5,
        _ => 0,
    }
}

/// The graphics view accepts trailing spaces after the closing dollar.
pub fn latex_caption(caption: &str) -> Option<&str> {
    caption
        .strip_prefix('$')?
        .trim_end_matches(|ch| ch <= ' ')
        .strip_suffix('$')
}

impl Construction {
    pub fn set_number_rounding(&mut self, rounding: NumberRounding) {
        self.number_rounding = rounding;
    }

    pub fn set_slider_label(&mut self, id: Id, label: SliderLabel) {
        self.slider_labels.insert(id, label);
    }

    pub fn input_box_rounded_number(&self, id: Id, link: Id) -> Option<String> {
        let rounding = self.appearance(id).input_rounding?;
        let Value::Number(value) = self.value(link) else {
            return None;
        };
        let angle = self.kind(link) == crate::Kind::Angle;
        let mut text = String::new();
        rounding.write(&mut text, if angle { value.to_degrees() } else { value });
        if angle {
            text.push('°');
        }
        Some(text)
    }

    pub fn format_display_number(&self, value: f64) -> String {
        let mut out = String::new();
        self.number_rounding.write(&mut out, value);
        out
    }

    pub fn slider_label(&self, id: Id, value: f64) -> Cow<'_, str> {
        let name = self.label(id);
        let saved = self.slider_labels.get(&id);
        let edited = self.appearance(id);
        let mode = edited
            .label_mode()
            .unwrap_or_else(|| saved.map_or(1, |label| label.mode));
        let caption = || {
            let text = edited
                .caption
                .as_deref()
                .or_else(|| saved.map(|label| label.caption.as_ref()));
            self.expand_caption(id, text.unwrap_or_default())
        };
        let prefix = match if mode == -1 {
            1
        } else {
            resolve_label_mode(mode, 1)
        } {
            1 => Cow::Borrowed(name),
            2 => return Cow::Owned(self.format_display_number(value)),
            3 => return caption(),
            9 => caption(),
            _ => return Cow::Borrowed(name),
        };
        let mut out = match prefix {
            Cow::Borrowed(prefix) => {
                let mut out = String::with_capacity(prefix.len() + 24);
                out.push_str(prefix);
                out
            }
            Cow::Owned(prefix) => prefix,
        };
        out.push_str(" = ");
        self.number_rounding.write(&mut out, value);
        Cow::Owned(out)
    }

    pub fn checkbox_caption(&self, id: Id) -> Cow<'_, str> {
        self.object_caption(id)
    }

    pub fn object_caption(&self, id: Id) -> Cow<'_, str> {
        let caption = self
            .appearance(id)
            .caption
            .as_deref()
            .or_else(|| self.control(id).map(|control| control.caption.as_str()))
            .unwrap_or_default();
        self.expand_caption(id, caption)
    }

    pub fn next_cell_tooltip(&self, id: Id) -> Option<Cow<'_, str>> {
        let next = next_cell_in_first_row(self.label(id)).and_then(|label| self.id(&label))?;
        Some(self.caption_value(next))
    }

    /// Expands the saved caption's substitutions and falls back to the object name.
    pub fn expand_caption<'a>(&'a self, id: Id, caption: &'a str) -> Cow<'a, str> {
        // Caption substitutions can repeat long object values. Cap expansion
        // work and use the literal caption if the result exceeds the cap.
        const MAX_EXPANSION: usize = 16_384;

        let name = self.label(id);
        if caption.is_empty() {
            return Cow::Borrowed(name);
        }
        if caption.len() > MAX_EXPANSION || !caption.contains('%') {
            return Cow::Borrowed(caption);
        }
        let mut out = String::with_capacity(caption.len());
        let mut rest = caption;
        while let Some((literal, suffix)) = rest.split_once('%') {
            out.push_str(literal);
            let Some(code) = suffix.chars().next() else {
                rest = "%";
                break;
            };
            rest = &suffix[code.len_utf8()..];
            let value = match code {
                'n' => Cow::Borrowed(name),
                'v' | 'f' | 'd' => self.caption_value(id),
                'c' => next_cell_in_first_row(name)
                    .and_then(|label| self.id(&label))
                    .map_or(Cow::Borrowed(""), |cell| self.caption_value(cell)),
                _ => {
                    out.push('%');
                    out.push(code);
                    continue;
                }
            };
            if out.len() + value.len() > MAX_EXPANSION {
                return Cow::Borrowed(caption);
            }
            out.push_str(&value);
        }
        if out.len() + rest.len() > MAX_EXPANSION {
            return Cow::Borrowed(caption);
        }
        out.push_str(rest);
        if out.is_empty() {
            Cow::Borrowed(name)
        } else {
            Cow::Owned(out)
        }
    }

    fn caption_value(&self, id: Id) -> Cow<'_, str> {
        match self.value(id) {
            Value::Number(value) => Cow::Owned(self.format_display_number(value)),
            Value::Bool(true) => Cow::Borrowed("true"),
            Value::Bool(false) => Cow::Borrowed("false"),
            Value::Text => Cow::Borrowed(self.text(id).unwrap_or_default()),
            Value::Undefined => Cow::Borrowed("?"),
            _ => Cow::Borrowed(self.label(id)),
        }
    }
}

/// CaptionBuilder's %c reads row 1 in the next spreadsheet column.
fn next_cell_in_first_row(label: &str) -> Option<String> {
    let (column, row) = label.split_at(label.find(|ch: char| ch.is_ascii_digit())?);
    if column.is_empty()
        || column.len() > 3
        || !column.bytes().all(|b| b.is_ascii_uppercase())
        || row.parse::<u32>().ok()? == 0
    {
        return None;
    }
    let mut column = column.as_bytes().to_vec();
    for digit in column.iter_mut().rev() {
        if *digit < b'Z' {
            *digit += 1;
            column.push(b'1');
            return String::from_utf8(column).ok();
        }
        *digit = b'A';
    }
    column.extend_from_slice(b"A1");
    String::from_utf8(column).ok()
}

impl NumberRounding {
    fn write(self, out: &mut String, value: f64) {
        match value {
            _ if value.is_nan() => out.push('?'),
            f64::INFINITY => out.push('∞'),
            f64::NEG_INFINITY => out.push_str("-∞"),
            std::f64::consts::PI => out.push('π'),
            _ => match self {
                Self::Decimals(_) if value.abs() >= (1u64 << 53) as f64 => {
                    write_scientific(out, &format!("{value:.14e}"), true);
                }
                Self::Decimals(places) => write_decimal(out, value, places.min(15)),
                Self::SignificantFigures(figures) => {
                    let digits = figures.clamp(1, 15);
                    let precision = (digits - 1) as usize;
                    let value = value + 0.0;
                    let mut scientific = format!("{value:.precision$e}");
                    let mut position = decimal_position(&scientific);
                    if is_decimal_midpoint(value.abs(), digits as i32 - position) {
                        // Rust formats exact ties to even. The reference rounds
                        // them away from zero, so move just that tie by one ulp.
                        let above = value.abs().next_up().copysign(value);
                        scientific = format!("{above:.precision$e}");
                        position = decimal_position(&scientific);
                    }
                    if !(digits as i32 - 15..=16).contains(&position) {
                        write_scientific(out, &scientific, false);
                        return;
                    }
                    let (mantissa, _) = scientific.split_once('e').unwrap();
                    let (sign, mantissa) = mantissa
                        .strip_prefix('-')
                        .map_or(("", mantissa), |m| ("-", m));
                    out.push_str(sign);
                    if position <= 0 {
                        out.push_str("0.");
                        out.extend(std::iter::repeat_n('0', (-position) as usize));
                    }
                    for (i, digit) in mantissa.chars().filter(|&ch| ch != '.').enumerate() {
                        if i != 0 && i as i32 == position {
                            out.push('.');
                        }
                        out.push(digit);
                    }
                    out.extend(std::iter::repeat_n(
                        '0',
                        position.saturating_sub(digits as i32).max(0) as usize,
                    ));
                }
            },
        }
    }
}

fn decimal_position(scientific: &str) -> i32 {
    scientific
        .split_once('e')
        .unwrap()
        .1
        .parse::<i32>()
        .unwrap()
        + 1
}

fn is_decimal_midpoint(value: f64, places: i32) -> bool {
    match places {
        // Powers of ten through 22 are exact doubles. Beyond 22 the
        // midpoint's factor of 5 alone needs more than 53 bits.
        0..=22 => {
            let scale = 10_u128.pow(places as u32) as f64;
            let scaled = value * scale;
            scaled.fract() == 0.5 && value.mul_add(scale, -scaled) == 0.0
        }
        -22..=-1 => {
            let scale = 10_u128.pow(places.unsigned_abs()) as f64;
            value % scale == scale / 2.0
        }
        _ => false,
    }
}

/// DecimalFormat's half-up rule for a binary value. The fused difference
/// disambiguates a product that rounded to a half during scaling.
pub(crate) fn write_decimal(out: &mut String, value: f64, places: u32) {
    let scale = 10_u64.pow(places);
    let absolute = value.abs();
    let fraction = absolute.fract();
    let scaled = fraction * scale as f64;
    let rounded = if scaled.fract() == 0.5 && fraction.mul_add(scale as f64, -scaled) < 0.0 {
        scaled.floor()
    } else {
        scaled.round()
    } as u64;
    let whole = absolute as u64 + rounded / scale;
    let mut fractional = rounded % scale;
    if value.is_sign_negative() && (whole != 0 || fractional != 0) {
        out.push('-');
    }
    let _ = write!(out, "{whole}");
    if fractional != 0 {
        let mut digits = places as usize;
        while fractional.is_multiple_of(10) {
            fractional /= 10;
            digits -= 1;
        }
        let _ = write!(out, ".{fractional:0digits$}");
    }
}

fn write_scientific(out: &mut String, scientific: &str, trim_zeros: bool) {
    const SUPERSCRIPT: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    out.push_str(if trim_zeros {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    });
    out.push_str(" ⋅ 10");
    out.extend(exponent.bytes().map(|digit| {
        if digit == b'-' {
            '⁻'
        } else {
            SUPERSCRIPT[usize::from(digit - b'0')]
        }
    }));
}
