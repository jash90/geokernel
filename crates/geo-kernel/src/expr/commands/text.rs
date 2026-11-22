// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Kernel.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionNodeEvaluator.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDependentText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoVerticalText.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

mod format;
mod numeric;
pub(crate) mod strings;
mod table;

use std::ops::Range;

use crate::expr::eval::{Ctx, Val};
use crate::expr::parse::Parser;
use crate::expr::{types, Elem, Node, Ty, NONE};
use crate::kernel::text::{TextPosition, TextPresentation};
use crate::kernel::{Construction, Def, Kind, Value};

/// Bound the result of concatenation, replacement or the expansion of a list.
const MAX_TEXT_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextOp {
    Display,
    Formula,
    Vertical,
    Rotate,
    Ordinal,
    Fraction,
    Scientific,
    Surd,
    Continued,
    Table,
    LetterToUnicode,
    UnicodeToLetter,
    TextToUnicode,
    UnicodeToText,
    ReplaceAll,
    Split,
    IndexOf,
    Length,
    Take,
    Sum,
    /// `First(text[, n])` of a text (`AlgoFirstString`).
    First,
    /// `Last(text[, n])` of a text (`AlgoLastString`).
    Last,
    /// `Element(text, n)`: one character of a text (`AlgoTextElement`).
    TextElement,
}

pub(super) const NAMES: &[&str] = &[
    "Text",
    "FormulaText",
    "LaTeX",
    "VerticalText",
    "RotateText",
    "Ordinal",
    "FractionText",
    "ScientificText",
    "SurdText",
    "ContinuedFraction",
    "TableText",
    "LetterToUnicode",
    "UnicodeToLetter",
    "TextToUnicode",
    "UnicodeToText",
    "ReplaceAll",
    "Split",
    "IndexOf",
    "Length",
    "Take",
    "Name",
    "Sum",
];

pub(super) fn parse(p: &mut Parser<'_>, name: &str) -> Result<Option<u32>, String> {
    let op = match name {
        "Text" => TextOp::Display,
        "FormulaText" | "LaTeX" => TextOp::Formula,
        "VerticalText" => TextOp::Vertical,
        "RotateText" => TextOp::Rotate,
        "Ordinal" => TextOp::Ordinal,
        "FractionText" => TextOp::Fraction,
        "ScientificText" => TextOp::Scientific,
        "SurdText" => TextOp::Surd,
        "ContinuedFraction" => TextOp::Continued,
        "TableText" => TextOp::Table,
        "LetterToUnicode" => TextOp::LetterToUnicode,
        "UnicodeToLetter" => TextOp::UnicodeToLetter,
        "TextToUnicode" => TextOp::TextToUnicode,
        "UnicodeToText" => TextOp::UnicodeToText,
        "ReplaceAll" => TextOp::ReplaceAll,
        "Split" => TextOp::Split,
        "IndexOf" => TextOp::IndexOf,
        "Length" => TextOp::Length,
        "Take" => TextOp::Take,
        "Name" => return parse_name(p),
        // `Sum` of texts concatenates them (`TextFold`); of one argument
        // the list itself, of several the wrapped arguments — but never
        // two (`CmdSum` wants a list first). Anything else is the
        // statistics group's command, so it must fall through.
        "Sum" => {
            return p.list_command(|p, args, _| {
                let texty = |p: &Parser<'_>, i: u32| {
                    p.is_text(i) || p.list_element_type(i) == Some(Elem::Text)
                };
                if args.is_empty() || !args.iter().all(|&a| texty(p, a)) {
                    return Ok(None);
                }
                if args.len() == 2 || (args.len() == 1 && p.is_text(args[0])) {
                    return Ok(None);
                }
                Ok(Some(call_node(p, TextOp::Sum, args)))
            });
        }
        "First" => TextOp::First,
        "Last" => TextOp::Last,
        "Element" => TextOp::TextElement,
        _ => return Ok(None),
    };
    if matches!(
        op,
        TextOp::Length
            | TextOp::IndexOf
            | TextOp::Take
            | TextOp::First
            | TextOp::Last
            | TextOp::TextElement
    ) {
        return p.list_command(|p, args, _| {
            // `IndexOf` takes its text form only when the haystack is a
            // text too (`CmdIndexOf`); the others read their first argument
            let text_at = |i: usize| args.get(i).is_some_and(|&a| p.is_text(a));
            let takes = match op {
                TextOp::IndexOf => text_at(0) && text_at(1),
                _ => text_at(0),
            };
            if takes {
                return Ok(Some(call_node(p, op, args)));
            }
            Ok(None)
        });
    }
    let args = p.arguments()?;
    let node = call_node(p, op, &args);
    Ok(Some(p.push(node)))
}

/// `Name(g)`: the label of the object `g` as a fixed string (`AlgoName`).
/// An object the construction keeps hidden has no label to show; its
/// formula string is not reproduced here.
fn parse_name(p: &mut Parser<'_>) -> Result<Option<u32>, String> {
    let args = p.arguments()?;
    match args[..] {
        [arg] => match p.nodes[arg as usize] {
            Node::Obj(id) => {
                let label = p.label(id).to_owned();
                if label.starts_with('\0') {
                    return Err("`Name` needs an object with a label".into());
                }
                Ok(Some(p.text_literal(&label)))
            }
            _ => Err("`Name` needs an object".into()),
        },
        _ => Err(format!("`Name` does not take {} argument(s)", args.len())),
    }
}

fn call_node(p: &mut Parser<'_>, op: TextOp, args: &[u32]) -> Node {
    let first = p.nodes.len() as u32;
    for &arg in args {
        p.push(Node::Item(arg));
    }
    Node::TextCmd(op, first, args.len() as u32)
}

pub(in crate::expr) fn result_type(
    c: &Construction,
    op: TextOp,
    nodes: &[Node],
    types: &[Ty],
    range: Range<usize>,
) -> Result<Ty, String> {
    use TextOp::*;
    use Ty::{Bool, Text, Vec};
    let args = &types[range.clone()];
    let point =
        |i: usize| types::vec_kind(c, nodes, types, (range.start + i) as u32) == Kind::Point;
    let number = |t: Ty| matches!(t, Ty::Num(_));
    let list = |t: Ty| matches!(t, Ty::List(_));
    let valid = match (op, args) {
        (Display, [_] | [_, Bool]) => true,
        (Display, [_, Vec] | [_, Vec, Bool] | [_, Vec, Bool, Bool]) => point(1),
        (Display, [_, Vec, Bool, Bool, x]) => point(1) && number(*x),
        (Display, [_, Vec, Bool, Bool, x, y]) => point(1) && number(*x) && number(*y),
        (Formula, [_] | [_, Bool] | [_, Bool, Bool]) => true,
        (Vertical, [Text]) => true,
        (Vertical, [Text, Vec]) => point(1),
        (Rotate, [Text, angle]) => number(*angle),
        (Rotate, [Text, angle, Vec]) => number(*angle) && point(2),
        (Fraction | Surd, [Vec]) => point(0),
        (Ordinal | Scientific | Surd | Continued | UnicodeToLetter, [x]) => number(*x),
        (Fraction, [x] | [x, Bool]) => number(*x),
        (Scientific, [x, y]) => number(*x) && number(*y),
        (Surd, [x, y]) => number(*x) && list(*y),
        (Continued, [x, Bool]) => number(*x),
        (Continued, [x, y] | [x, y, Bool]) => number(*x) && number(*y),
        (Table, args) => table::valid_types(args),
        (Length | LetterToUnicode | TextToUnicode, [Text]) => true,
        (UnicodeToText, [a]) | (Split, [Text, a]) => list(*a),
        (ReplaceAll, [Text, Text, Text]) => true,
        (Sum, [Text, ..]) => true,
        (Sum, [Ty::List(Elem::Text)]) => true,
        (IndexOf, [Text, Text]) => true,
        (IndexOf, [Text, Text, n]) | (Take, [Text, n]) => number(*n),
        (Take, [Text, a, b]) => number(*a) && number(*b),
        (First | Last, [Text]) => true,
        (First | Last, [Text, n]) => number(*n),
        (TextElement, [Text, n]) => number(*n),
        _ => false,
    };
    if !valid {
        return Err("invalid arguments to the text command".into());
    }
    Ok(match op {
        LetterToUnicode | Length | IndexOf => Ty::Num(0),
        TextToUnicode => Ty::List(Elem::Num),
        Split => Ty::List(Elem::Text),
        _ => Text,
    })
}

impl<const M: bool> Ctx<'_, M> {
    fn text_args(&self, first: u32, count: u32) -> impl Iterator<Item = u32> + '_ {
        (first..first + count).map(|i| match self.node(i) {
            Node::Item(arg) => arg,
            _ => unreachable!("a text argument is wrapped in an item node"),
        })
    }

    fn bounded_text(&self, size: usize) -> Option<()> {
        (size <= MAX_TEXT_BYTES && self.meter.is_none_or(|m| m.charge(size as u64))).then_some(())
    }

    pub(crate) fn concatenate(&self, a: u32, b: u32) -> Option<String> {
        let mut left = self.value_string(a);
        let right = self.value_string(b);
        self.bounded_text(left.len().checked_add(right.len())?)?;
        left.push_str(&right);
        Some(left)
    }

    pub(crate) fn value_string(&self, i: u32) -> String {
        let latex = self.text_latex();
        if self.e.tys[i as usize] == Ty::Text {
            // An undefined GeoText has an empty value string.
            return self
                .text(i)
                .map_or_else(String::new, std::borrow::Cow::into_owned);
        }
        if matches!(self.e.tys[i as usize], Ty::List(_)) {
            return format::list_value(self, i, latex).unwrap_or_else(|| "?".into());
        }
        if let Node::Obj(id) | Node::FunctionObject(id) = self.node(i) {
            let v = self.c.value(id);
            if !matches!(v, Value::Number(_) | Value::Undefined) {
                return format::object_value(self, id, latex).unwrap_or_else(|| "?".into());
            }
        }
        let evaluated = self.full(i);
        match evaluated.val {
            Val::N(v) if self.e.tys[i as usize] == Ty::Num(1) && !v.is_nan() => format_angle(v),
            Val::N(v) if v == std::f64::consts::PI => format_number_with_template(v, latex),
            Val::N(v) => {
                let rounded = evaluated
                    .dec
                    .as_ref()
                    .and_then(|exact| exact.get(v))
                    .and_then(|decimal| decimal.set_scale_half_up(2))
                    .map_or(v, |decimal| decimal.to_f64());
                format_number_with_template(rounded, latex)
            }
            Val::V(x, y) if x.is_finite() && y.is_finite() => format::coordinates(x, y, latex),
            Val::B(b, true) => b.to_string(),
            _ => "?".into(),
        }
    }

    pub(crate) fn text_command(&self, op: TextOp, first: u32, count: u32) -> Option<String> {
        use TextOp::*;
        let args: Vec<_> = self.text_args(first, count).collect();
        let a = args[0];
        let b = args.get(1).copied();
        let c = args.get(2).copied();
        let flag = |i: u32| matches!(self.full(i).val, Val::B(true, _));
        let number = |i| self.full_number(i);
        let result = match op {
            Display | Formula | Vertical | Rotate => format::evaluate(self, op, &args),
            Ordinal => numeric::ordinal(number(a)),
            Fraction if self.e.tys[a as usize] == Ty::Vec => match self.full(a).val {
                Val::V(x, y) => Some(numeric::fraction_point(x, y)),
                _ => Some(numeric::fraction(f64::NAN, true)),
            },
            Fraction => Some(numeric::fraction(number(a), b.is_none_or(flag))),
            Scientific => numeric::scientific(number(a), b.map(number)),
            Surd if self.e.tys[a as usize] == Ty::Vec => Some(numeric::surd_point(self, a)),
            Surd => numeric::surd(self, a, b.unwrap_or(NONE)),
            Continued => {
                let boolean = b.filter(|&i| self.e.tys[i as usize] == Ty::Bool);
                let level = b.filter(|i| Some(*i) != boolean).map(number);
                numeric::continued(number(a), level, c.or(boolean).is_some_and(flag))
            }
            Table => table::evaluate(self, &args),
            UnicodeToLetter | UnicodeToText | ReplaceAll | Take | Sum | First | Last
            | TextElement => strings::evaluate(self, op, &args),
            _ => None,
        }?;
        self.bounded_text(result.len())?;
        Some(result)
    }

    pub(crate) fn text_is_latex(&self, i: u32) -> bool {
        match self.node(i) {
            Node::Obj(id) if self.c.kind(id) == Kind::Text => self
                .c
                .text_presentation(id)
                .is_some_and(|style| style.latex),
            Node::TextCmd(op, first, count) => self.text_style(op, first, count).latex,
            _ => false,
        }
    }

    pub(in crate::expr) fn text_style(
        &self,
        op: TextOp,
        first: u32,
        count: u32,
    ) -> TextPresentation {
        let args: Vec<_> = self.text_args(first, count).collect();
        let mut s = TextPresentation::default();
        if op == TextOp::Table {
            s.position = TextPosition::Screen { x: 0.0, y: 0.0 };
        }
        let anchor = match op {
            TextOp::Display
                if args
                    .get(1)
                    .is_some_and(|&a| self.e.tys[a as usize] == Ty::Vec) =>
            {
                args.get(1)
            }
            TextOp::Vertical => args.get(1),
            TextOp::Surd => args.first().filter(
                |&&arg| matches!(self.node(arg), Node::Obj(id) if self.c.kind(id) == Kind::Point),
            ),
            _ => None,
        };
        if let Some(&anchor) = anchor {
            s.fixed = true;
            s.position = match self.node(anchor) {
                Node::Obj(id) if self.c.kind(id) == Kind::Point => TextPosition::Point(id),
                _ => match self.full(anchor).val {
                    Val::V(x, y) if x.is_finite() && y.is_finite() => TextPosition::World { x, y },
                    _ => TextPosition::Undefined,
                },
            };
        }
        s.latex = match op {
            TextOp::Display => args
                .get(3)
                .is_some_and(|&arg| matches!(self.full(arg).val, Val::B(true, _))),
            TextOp::Formula
            | TextOp::Vertical
            | TextOp::Rotate
            | TextOp::Fraction
            | TextOp::Scientific
            | TextOp::Surd
            | TextOp::Continued
            | TextOp::Table => true,
            _ => false,
        };
        if op == TextOp::Rotate {
            s.serif = match &self.e.extra {
                crate::expr::StoredExtra::Text(result) if matches!(self.node(self.e.root), Node::TextCmd(_, f, _) if f == first) => {
                    result.presentation.serif
                }
                _ => match self.node(args[0]) {
                    Node::Obj(id) => self
                        .c
                        .text_presentation(id)
                        .is_some_and(|style| style.serif),
                    _ => false,
                },
            };
        }
        if op == TextOp::Display {
            let align = |i: usize| {
                args.get(i)
                    .map(|&a| self.full_number(a) as i32)
                    .filter(|v| (-1..=1).contains(v))
            };
            s.horizontal = align(4);
            s.vertical = align(5);
        }
        s
    }
}

pub(super) fn format_angle(value: f64) -> String {
    let degrees = value.to_degrees();
    let nearest = (degrees * 100_000.0).round();
    let degrees = if (degrees * 100_000.0 - nearest).abs() < 1e-7 {
        nearest / 100_000.0
    } else {
        degrees
    };
    format!("{}°", format_number(degrees))
}

pub(super) fn format_number(value: f64) -> String {
    format_number_with_template(value, false)
}

// StringTemplate uses its own pi and scientific notation for LaTeX.
fn format_number_with_template(value: f64, latex: bool) -> String {
    if value.is_nan() {
        return "?".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-∞"
        } else {
            "∞"
        }
        .into();
    }
    if value == std::f64::consts::PI {
        return if latex { r"\pi " } else { "π" }.into();
    }
    if value.abs() >= (1u64 << 53) as f64 {
        const SUPERSCRIPT: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
        let scientific = format!("{value:.14e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        if latex {
            return format!(r"{mantissa} \cdot 10^{{{exponent}}}");
        }
        let mut formatted = mantissa.to_owned();
        formatted.push_str(" ⋅ 10");
        formatted.extend(
            exponent
                .bytes()
                .map(|digit| SUPERSCRIPT[usize::from(digit - b'0')]),
        );
        return formatted;
    }
    // Java DecimalFormat rounds a binary value at a true half away from
    // zero. Recover the direction of the rounding error if x*100 rounded
    // to a half in the multiplication.
    let absolute = value.abs();
    let fraction = absolute.fract();
    let scaled = fraction * 100.0;
    let rounded = if scaled.fract() == 0.5 && fraction.mul_add(100.0, -scaled) < 0.0 {
        scaled.floor()
    } else {
        scaled.round()
    };
    let cents = (absolute as u64) * 100 + rounded as u64;
    let sign = if value.is_sign_negative() && cents != 0 {
        "-"
    } else {
        ""
    };
    match cents % 100 {
        0 => format!("{sign}{}", cents / 100),
        fractional if fractional % 10 == 0 => format!("{sign}{}.{}", cents / 100, fractional / 10),
        fractional => format!("{sign}{}.{fractional:02}", cents / 100),
    }
}

fn format_coords(x: f64, y: f64) -> String {
    format!("({}, {})", format_number(x), format_number(y))
}

pub(super) fn format_value(value: Value) -> String {
    let number = match value {
        Value::Number(v) => v,
        Value::Point { x, y } | Value::Vector { x, y } => return format_coords(x, y),
        Value::Bool(b) => return b.to_string(),
        Value::Segment { x1, y1, x2, y2 } => (x2 - x1).hypot(y2 - y1),
        Value::Polygon { area, .. } => area.abs(),
        Value::PolyLine { length, .. } => length,
        Value::Arc { length } => length,
        Value::Sector { area, .. } => area,
        _ => f64::NAN,
    };
    format_number(number)
}

pub(in crate::expr) fn is_independent(c: &Construction, id: crate::Id) -> bool {
    let Def::Expr(d) = c.def(id) else {
        return false;
    };
    let entry = crate::expr::entry(c, d);
    d.kind() == Kind::Text
        && entry.parents.is_empty()
        && !entry.nodes.iter().any(|n| {
            matches!(
                n,
                Node::TextCmd(..)
                    | Node::Relation { .. }
                    | Node::Base { .. }
                    | Node::Cmd(..)
                    | Node::If(crate::expr::IfMode::Algo, ..)
                    | Node::ListOp(..)
            )
        })
}
