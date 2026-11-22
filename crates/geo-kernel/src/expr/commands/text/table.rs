// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTableText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdTableText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/StringTemplate.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use crate::expr::eval::Ctx;
use crate::expr::list::Slot;
use crate::expr::Ty;
use crate::{Kind, Value};
use std::fmt::Write as _;

pub(super) fn valid_types(args: &[Ty]) -> bool {
    if !matches!(args.first(), Some(Ty::List(_))) {
        return false;
    }
    let count = args.iter().take_while(|t| matches!(t, Ty::List(_))).count();
    let rest = &args[count..];
    let numbers = rest.strip_prefix(&[Ty::Text]).unwrap_or(rest);
    numbers.len() <= 2 && numbers.iter().all(|t| matches!(t, Ty::Num(_)))
}

pub(super) fn evaluate<const M: bool>(ctx: &Ctx<'_, M>, args: &[u32]) -> Option<String> {
    let mut lists = args
        .iter()
        .copied()
        .take_while(|&i| matches!(ctx.e.tys[i as usize], Ty::List(_)));
    let first = lists.next()?;
    let Some(first) = ctx.list(first) else {
        return Some(String::new());
    };
    let mut runs = vec![first];
    for list in lists {
        runs.push(ctx.list(list).unwrap_or_default());
    }
    let remaining = &args[runs.len()..];
    let (options, dimensions) = match remaining {
        [first, rest @ ..] if ctx.e.tys[*first as usize] == Ty::Text => (ctx.text(*first), rest),
        _ => (None, remaining),
    };
    let mut options = options
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_default();
    let lists = ctx.lists?;
    let read = |index| lists.read(ctx.c, index);
    let first_is_nested = first.1 != 0 && read(first.0).kind == Kind::List;
    let width = dimensions.first().map(|&i| ctx.full_number(i));
    let mut size = width;
    let height = dimensions.get(1).map(|&i| ctx.full_number(i));
    if runs.len() == 1 {
        let has_options = remaining.len() != dimensions.len();
        let split = dimensions.len() == 1 && (!has_options || (first.1 != 0 && !first_is_nested));
        if split {
            let max_size = width? as u32;
            if max_size == 0 {
                return None;
            }
            runs.clear();
            for offset in (0..first.1).step_by(max_size as usize) {
                if ctx.stop(1) {
                    return None;
                }
                runs.push((first.0 + offset, max_size.min(first.1 - offset)));
            }
            size = None;
        } else if first.1 != 0 {
            // A matrix can carry its legacy options in a final text cell.
            let (start, len) = first;
            if ctx.stop(u64::from(len)) {
                return None;
            }
            if (start..start + len).any(|i| read(i).kind == Kind::List) {
                let last = read(start + len - 1);
                let tail = u32::from(last.kind == Kind::Text);
                if let Some(legacy) = last.text_value().filter(|_| options.is_empty()) {
                    options = legacy
                        .chars()
                        .skip_while(|c| !is_justification(*c))
                        .take_while(|c| is_justification(*c))
                        .collect();
                }
                runs.clear();
                for i in start..start + len - tail {
                    let cell = read(i);
                    if cell.kind != Kind::List {
                        return Some(String::new());
                    }
                    runs.push(cell.run().unwrap_or_default());
                }
            }
        }
    }

    let options = Options::new(&options);
    let longest = runs.iter().map(|&(_, n)| n).max().unwrap_or(0);
    if longest == 0 {
        return Some(String::new());
    }
    let (rows, cols) = if options.vertical {
        (longest, runs.len() as u32)
    } else {
        (runs.len() as u32, longest)
    };
    let cells = u64::from(rows) * u64::from(cols);
    if cells > super::MAX_TEXT_BYTES as u64 || ctx.stop(cells) {
        return None;
    }
    let mut out = String::from("{");
    out.push_str(options.open);
    out.push_str("\\begin{array}{");
    let width = size.map(java_decimal);
    let height = height.map(java_decimal);
    for col in 0..cols {
        if options.vertical_line(col, cols) {
            out.push('|');
        }
        let justification = options.justification(col);
        let latex = if matches!(justification, 'l' | 'c' | 'r') {
            justification
        } else {
            'r'
        };
        if let Some(width) = &width {
            write!(out, "{}{{{width}}}", justification.to_ascii_uppercase()).unwrap();
        } else {
            out.push(latex);
        }
    }
    if options.vertical_line(cols, cols) {
        out.push('|');
    }
    out.push('}');
    if options.horizontal_line(0, rows) {
        out.push_str("\\hline ");
    }
    for row in 0..rows {
        for col in 0..cols {
            let (run, cell) = if options.vertical {
                (col, row)
            } else {
                (row, col)
            };
            let (start, len) = runs[run as usize];
            if cell < len {
                let slot = read(start + cell);
                write_cell(ctx, &mut out, &slot, options.justification(run), &options)?;
            }
            if col + 1 < cols {
                out.push('&');
            }
        }
        if let Some(height) = &height {
            write!(out, "\\\\[{height}]").unwrap();
        } else {
            out.push_str(" \\\\ ");
        }
        if out.len() > super::MAX_TEXT_BYTES {
            return None;
        }
        if options.horizontal_line(row + 1, rows) {
            out.push_str("\\hline ");
        }
    }
    out.push_str("\\end{array}");
    out.push_str(options.close);
    out.push('}');
    Some(out)
}

fn is_justification(c: char) -> bool {
    ".%lrcap".contains(c)
}

struct Options<'a> {
    vertical: bool,
    justification: &'a str,
    v_lines: Option<&'a str>,
    h_lines: Option<&'a str>,
    v_edges: bool,
    h_edges: bool,
    decimal_comma: bool,
    currency: Option<(char, bool)>,
    open: &'static str,
    close: &'static str,
}

impl<'a> Options<'a> {
    fn new(options: &'a str) -> Self {
        let justification = options.trim_start_matches(|c| !is_justification(c));
        let end = justification
            .find(|c| !is_justification(c))
            .unwrap_or(justification.len());
        let (open, close) = brackets(options);
        let line_flags = |c| {
            options.split_once(c).map(|(_, rest)| {
                let end = rest.find(|ch| ch != '0' && ch != '1').unwrap_or(rest.len());
                &rest[..end]
            })
        };
        let currency_symbol = |ch: &char| "฿$₫€₹£₨₪₮₩¥".contains(*ch);
        let currency = options
            .chars()
            .next()
            .filter(currency_symbol)
            .map(|ch| (ch, true))
            .or_else(|| {
                options
                    .chars()
                    .next_back()
                    .filter(currency_symbol)
                    .map(|ch| (ch, false))
            });
        Self {
            vertical: options.contains('v'),
            justification: &justification[..end],
            v_lines: if options.contains("||") {
                None
            } else {
                line_flags('|')
            },
            h_lines: line_flags('_'),
            v_edges: options.contains('/'),
            h_edges: options.contains('-'),
            decimal_comma: options.contains(','),
            currency,
            open,
            close,
        }
    }

    fn justification(&self, index: u32) -> char {
        let bytes = self.justification.as_bytes();
        char::from(
            *bytes
                .get(index as usize)
                .or_else(|| bytes.first())
                .unwrap_or(&b'l'),
        )
    }

    fn vertical_line(&self, index: u32, cols: u32) -> bool {
        line(self.v_lines, self.v_edges, index, cols)
    }

    fn horizontal_line(&self, index: u32, rows: u32) -> bool {
        line(self.h_lines, self.h_edges, index, rows)
    }
}

fn line(flags: Option<&str>, edges: bool, index: u32, count: u32) -> bool {
    (!edges || index == 0 || index == count)
        && flags.is_some_and(|s| s.as_bytes().get(index as usize).copied() != Some(b'0'))
}

fn brackets(options: &str) -> (&'static str, &'static str) {
    let (mut open, mut close) = ("\\left.", "\\right.");
    for (candidate, left, right) in [
        ("||||", "\\left| \\left|", "\\right| \\right|"),
        ("||", "\\left|", "\\right|"),
        ("(", "\\left(", "\\right."),
        ("[", "\\left[", "\\right."),
        ("{", "\\left\\{", "\\right."),
    ] {
        if options.contains(candidate) {
            (open, close) = (left, right);
            break;
        }
    }
    for (candidate, right) in [(')', "\\right)"), (']', "\\right]"), ('}', "\\right\\}")] {
        if options.contains(candidate) {
            close = right;
            break;
        }
    }
    if (open, close) == ("\\left.", "\\right.") {
        ("", "")
    } else {
        (open, close)
    }
}

fn java_decimal(value: f64) -> String {
    if !value.is_finite() {
        return match value {
            f64::INFINITY => "Infinity",
            f64::NEG_INFINITY => "-Infinity",
            _ => "NaN",
        }
        .into();
    }
    let mut text = if value == 0.0 || (1e-3..1e7).contains(&value.abs()) {
        value.to_string()
    } else {
        format!("{value:e}").replace('e', "E")
    };
    let end = text.find('E').unwrap_or(text.len());
    if !text[..end].contains('.') {
        text.insert_str(end, ".0");
    }
    text
}

fn write_cell<const M: bool>(
    ctx: &Ctx<'_, M>,
    out: &mut String,
    slot: &Slot,
    justification: char,
    options: &Options<'_>,
) -> Option<()> {
    let mut value = String::new();
    slot_tex(ctx, slot, &mut value)?;
    let shown = slot.shown();
    if matches!(justification, '.' | 'a') {
        if matches!(shown, Value::Number(_)) {
            value = super::format_value(shown);
        }
        value = pad_decimal(&value, justification == '.', "");
    } else if let (Value::Number(n), '%' | 'p') = (shown, justification) {
        let percent = super::format_value(Value::Number(n * 100.0));
        value = pad_decimal(&percent, justification == '%', "%");
    }
    if options.decimal_comma {
        value = value.replace('.', ",");
    }
    if matches!(value.as_str(), " " | "") {
        value = "\\;".into();
    }
    if let Some((symbol, before)) = options.currency {
        let token = if symbol == '$' {
            "\\dollar ".into()
        } else {
            symbol.to_string()
        };
        if before {
            value.insert_str(0, &token);
        } else {
            value.push_str(&token);
        }
    }
    let style = slot.object_id().and_then(|id| ctx.c.text_presentation(id));
    if slot.kind == Kind::Text
        && style.is_none_or(|s| !s.latex)
        && value.contains(' ')
        && !value.contains(['^', '{', '}', '+', '-', '=', '\\'])
    {
        value = match style.map(|s| (s.bold, s.italic)) {
            Some((true, true)) => format!("\\textit{{\\textbf{{{value}}}}}"),
            Some((true, false)) => format!("\\textbf{{{value}}}"),
            Some((false, true)) => format!("\\textit{{{value}}}"),
            _ => format!("\\text{{{value}}}"),
        };
    } else if let Some(style) = style {
        if style.bold {
            value = format!("\\mathbf{{{value}}}");
        }
        if style.italic {
            value = format!("\\mathit{{{value}}}");
        }
    }
    if let Some(id) = slot
        .object_id()
        .filter(|&id| !ctx.c.label(id).starts_with('\0'))
    {
        let color = match ctx.c.kind(id) {
            Kind::Point => "4D4DFF",
            _ => "1C1C1F",
        };
        write!(out, "\\textcolor{{#{color}}}{{{value}}}").unwrap();
    } else {
        out.push_str(&value);
    }
    (out.len() <= super::MAX_TEXT_BYTES).then_some(())
}

fn pad_decimal(value: &str, phantom: bool, suffix: &str) -> String {
    let padding = if value.parse::<f64>().is_ok() {
        match value.split_once('.') {
            Some((_, fraction)) => "0".repeat(2_usize.saturating_sub(fraction.len())),
            None => ".00".into(),
        }
    } else if phantom {
        ".00".into()
    } else {
        String::new()
    };
    if phantom && !padding.is_empty() {
        format!("\\texttt{{{value}{suffix}\\phantom{{\\texttt{{{padding}}}}}}}")
    } else {
        format!("\\texttt{{{value}{padding}{suffix}}}")
    }
}

fn slot_tex<const M: bool>(ctx: &Ctx<'_, M>, slot: &Slot, out: &mut String) -> Option<()> {
    if ctx.stop(1) {
        return None;
    }
    if let Some(text) = slot.text_value() {
        if ctx.stop(text.len() as u64) {
            return None;
        }
        if slot
            .object_id()
            .and_then(|id| ctx.c.text_presentation(id))
            .is_some_and(|s| s.latex)
        {
            out.push_str(text);
        } else {
            out.push_str(&text.replace('$', "\\dollar"));
        }
    } else if let Some((start, len)) = slot.run() {
        let lists = ctx.lists?;
        out.push_str("\\left\\{");
        for index in 0..len {
            if index != 0 {
                out.push_str(",\\;");
            }
            slot_tex(ctx, &lists.read(ctx.c, start + index), out)?;
        }
        out.push_str("\\right\\}");
    } else {
        let shown = slot.shown();
        if let (Value::Number(v), Kind::Angle) = (shown, slot.kind) {
            out.push_str(&super::format_angle(v));
        } else {
            out.push_str(&value_tex(shown));
        }
    }
    (out.len() <= super::MAX_TEXT_BYTES).then_some(())
}

fn value_tex(value: Value) -> String {
    let number = |v| super::format_value(Value::Number(v));
    match value {
        Value::Number(v) if v == std::f64::consts::PI => "\\pi ".into(),
        Value::Point { x, y } => format!("\\left({},\\;{} \\right)", number(x), number(y)),
        Value::Vector { x, y } => format!(
            "\\left( \\begin{{align}}{} \\\\ {} \\end{{align}} \\right)",
            number(x),
            number(y)
        ),
        _ => super::format_value(value),
    }
}
