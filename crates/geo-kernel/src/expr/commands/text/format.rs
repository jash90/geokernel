// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLaTeX.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoRotateText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoVerticalText.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoElement.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

mod expression;
mod object;

use super::{format_angle, format_number_with_template, format_value, TextOp, MAX_TEXT_BYTES};
use crate::algo::polygon::{Count, PolygonDef};
use crate::algo::vector::VectorDef;
use crate::expr::eval::{Ctx, Val};
use crate::expr::{self, Node, Ty};
use crate::kernel::{Def, Id, Kind, Value};

pub(super) fn evaluate<const M: bool>(
    ctx: &Ctx<'_, M>,
    op: TextOp,
    args: &[u32],
) -> Option<String> {
    let input = args[0];
    match op {
        TextOp::Display | TextOp::Formula => describe(ctx, op, args),
        TextOp::Vertical | TextOp::Rotate => {
            let degrees = if op == TextOp::Rotate {
                ctx.full_number(args[1]) * 180.0 / std::f64::consts::PI
            } else {
                90.0
            };
            if !degrees.is_finite()
                || op == TextOp::Vertical && args.get(1).is_some_and(|&i| !defined(ctx, i))
            {
                return Some(String::new());
            }
            let Some(text) = ctx.text(input) else {
                return Some(String::new());
            };
            if ctx.stop(text.len() as u64) {
                return None;
            }
            let degrees = java_double(degrees);
            if ctx.text_is_latex(input) {
                Some(format!(r"\rotatebox{{{degrees}}}{{ {text} }}"))
            } else {
                let text = text.replace('\n', r"\cr ");
                Some(format!(r"\rotatebox{{{degrees}}}{{ \text{{ {text} }}  }}"))
            }
        }
        _ => unreachable!("a different text command"),
    }
}

// Double.toString prints a decimal point for whole numbers and switches
// to scientific notation when the exponent is outside [-3, 7).
fn java_double(value: f64) -> String {
    if value != 0.0 && !(0.001..10_000_000.0).contains(&value.abs()) {
        let scientific = format!("{value:e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        let suffix = if mantissa.contains('.') { "" } else { ".0" };
        format!("{mantissa}{suffix}E{exponent}")
    } else {
        format!("{value:?}")
    }
}

fn defined<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> bool {
    if let Node::Obj(id) | Node::FunctionObject(id) = ctx.node(i) {
        return expr::object_defined(ctx.c, id);
    }
    match ctx.e.tys[i as usize] {
        Ty::Text => ctx.text(i).is_some(),
        Ty::List(_) => ctx.list(i).is_some(),
        _ => match ctx.full(i).val {
            Val::N(x) => !x.is_nan(),
            Val::V(x, y) => !x.is_nan() && !y.is_nan(),
            Val::B(_, defined) => defined,
        },
    }
}

fn describe<const M: bool>(ctx: &Ctx<'_, M>, op: TextOp, args: &[u32]) -> Option<String> {
    let input = args[0];
    let formula = op == TextOp::Formula;
    let id = match ctx.node(input) {
        Node::Obj(id) | Node::FunctionObject(id) => Some(id),
        _ => None,
    };
    let anchor = !formula
        && args
            .get(1)
            .is_some_and(|&i| ctx.e.tys[i as usize] == Ty::Vec);
    let substitute = args.get(1 + usize::from(anchor)).map_or(Some(true), |&i| {
        if let Val::B(value, true) = ctx.full(i).val {
            Some(value)
        } else {
            None
        }
    });
    let show_name = formula
        && args
            .get(2)
            .is_some_and(|&i| matches!(ctx.full(i).val, Val::B(true, _)));
    // AlgoText switches the flag after formatting its content.
    let format = Format::new(formula || ctx.text_latex());
    let text_input = ctx.e.tys[input as usize] == Ty::Text;
    if !formula && (anchor && !defined(ctx, args[1]) || !text_input && !defined(ctx, input)) {
        return None;
    }
    let Some(substitute) = substitute else {
        return formula.then(String::new);
    };
    if formula && args.get(2).is_some_and(|&i| !defined(ctx, i)) {
        return Some(String::new());
    }

    let mut value = if text_input {
        let text = ctx.text(input);
        let value = if formula {
            text.unwrap_or_default()
        } else {
            text?
        };
        if let Some(id) = id.filter(|&id| show_name && !ctx.c.label(id).starts_with('\0')) {
            if !ctx.text_is_latex(input) {
                return Some(String::new());
            }
            let table = matches!(ctx.c.def(id), Def::Expr(def)
                if matches!(expr::entry(ctx.c, def).nodes[expr::entry(ctx.c, def).root as usize],
                    Node::TextCmd(TextOp::Table, ..)));
            let value = if table {
                value.into_owned()
            } else {
                format.quoted(&value)
            };
            return Some(escape_percent(format!(
                r"{} \, = \,{value}",
                ctx.c.label(id)
            )));
        }
        value.into_owned()
    } else if let Some(id) = id {
        format.object(ctx, input, id, substitute)?
    } else if substitute {
        format.value(ctx, input)?
    } else {
        format.expression(ctx, input, false, 0)?
    };

    if let Some(id) = id.filter(|&id| show_name && !ctx.c.label(id).starts_with('\0')) {
        let name = Format::label(ctx.c.label(id), true);
        value = match ctx.c.kind(id) {
            Kind::Function => format!(r"{name}\left(x \right)\, = \,{value}"),
            Kind::Line | Kind::Circle | Kind::Conic | Kind::Ray => {
                format!(r"{name}\mathpunct{{:}}\,{value}")
            }
            _ => format!(r"{name}\, = \,{value}"),
        };
    }
    if format.latex {
        value = escape_percent(value);
    }
    Some(value)
}

fn escape_percent(value: String) -> String {
    if !value.contains('%') {
        return value;
    }
    let mut escaped = String::with_capacity(value.len());
    let mut previous = '\0';
    for ch in value.chars() {
        if ch == '%' && previous != '\\' {
            escaped.push('\\');
        }
        escaped.push(ch);
        previous = ch;
    }
    escaped
}

#[derive(Clone, Copy)]
struct Format {
    latex: bool,
    simplify_coefficients: bool,
}

pub(super) fn object_value<const M: bool>(ctx: &Ctx<'_, M>, id: Id, latex: bool) -> Option<String> {
    Format::new(latex).object_value(ctx, id)
}

pub(super) fn list_value<const M: bool>(ctx: &Ctx<'_, M>, i: u32, latex: bool) -> Option<String> {
    Format::new(latex).list(ctx, ctx.list(i), 0)
}

pub(super) fn coordinates(x: f64, y: f64, latex: bool) -> String {
    let format = Format::new(latex);
    format.pair(&format.number(x), &format.number(y))
}

impl Format {
    fn new(latex: bool) -> Self {
        Self {
            latex,
            simplify_coefficients: false,
        }
    }

    fn parenthesize(self, value: impl std::fmt::Display) -> String {
        if self.latex {
            format!(r"\left({value} \right)")
        } else {
            format!("({value})")
        }
    }

    fn pair(self, x: &str, y: &str) -> String {
        let separator = if self.latex { r",\;" } else { ", " };
        self.parenthesize(format_args!("{x}{separator}{y}"))
    }

    fn command(self, name: &str, args: &[String]) -> String {
        format!("{name}{}", self.parenthesize(args.join(", ")))
    }

    fn label(label: &str, latex: bool) -> String {
        if latex {
            label.replace('$', r"\$")
        } else {
            label.to_owned()
        }
    }

    fn number(self, x: f64) -> String {
        match x {
            f64::INFINITY if self.latex => r"\infty".into(),
            f64::NEG_INFINITY if self.latex => r"-\infty".into(),
            _ => format_number_with_template(x, self.latex),
        }
    }

    fn vector(self, x: f64, y: f64) -> String {
        let (x, y) = (self.number(x), self.number(y));
        if self.latex {
            format!(r"\left( \begin{{align}}{x} \\ {y} \end{{align}} \right)")
        } else {
            self.pair(&x, &y)
        }
    }

    fn shown(self, kind: Kind, value: Value) -> String {
        match value {
            Value::Point { x, y } => self.pair(&self.number(x), &self.number(y)),
            Value::Vector { x, y } => self.vector(x, y),
            Value::Circle { cx, cy, r } => self.circle(cx, cy, r),
            Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => self.line(a, b, c, None),
            Value::Number(value) if kind == Kind::Angle => format_angle(value),
            Value::Number(value) => self.number(value),
            _ => format_value(value),
        }
    }

    fn value<const M: bool>(self, ctx: &Ctx<'_, M>, i: u32) -> Option<String> {
        match ctx.e.tys[i as usize] {
            Ty::Vec => {
                let Val::V(x, y) = ctx.full(i).val else {
                    return Some("?".into());
                };
                Some(if matches!(ctx.node(i), Node::Vector(_)) {
                    self.vector(x, y)
                } else if x.is_finite() && y.is_finite() {
                    self.pair(&self.number(x), &self.number(y))
                } else {
                    "?".into()
                })
            }
            Ty::Num(1) => {
                let value = ctx.full_number(i);
                Some(if value.is_finite() {
                    format_angle(value)
                } else {
                    self.number(value)
                })
            }
            Ty::Num(_) => Some(self.number(ctx.full_number(i))),
            Ty::List(_) => self.list(ctx, ctx.list(i), 0),
            _ => Some(ctx.value_string(i)),
        }
    }

    fn object<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        input: u32,
        id: Id,
        substitute: bool,
    ) -> Option<String> {
        let kind = ctx.c.kind(id);
        let value = ctx.c.value(id);
        let def = ctx.c.def(id);
        if !substitute && matches!(def, Def::Equation(_)) {
            return self.user_equation::<M>(ctx.c, id, ctx.meter, false);
        }
        if !substitute && (!self.latex || kind != Kind::Vector) {
            if let Def::Expr(def) = def {
                let entry = expr::entry(ctx.c, def);
                let nested = Ctx { e: entry, ..*ctx };
                return self.expression(&nested, entry.root, substitute, 0);
            }
            if let Some(source) = ctx.c.construction_source(id) {
                return self.source(ctx, source, 0);
            }
            if let Some(formula) = self.command_definition(ctx, def) {
                return Some(formula);
            }
        }
        if kind.is_number() && !value.is_defined() {
            return Some(self.number(expr::relation_number(ctx.c, id)));
        }
        if kind == Kind::List {
            return self.list(ctx, ctx.list(input), 0);
        }
        self.object_value(ctx, id)
    }

    fn command_definition<const M: bool>(self, ctx: &Ctx<'_, M>, def: Def) -> Option<String> {
        let print = |id| {
            let label = ctx.c.label(id);
            if label.starts_with('\0') {
                self.shown(ctx.c.kind(id), ctx.c.value(id))
            } else {
                Self::label(label, self.latex)
            }
        };
        let (name, a, b) = match def {
            Def::Line(a, b) => ("Line", a, b),
            Def::Segment(a, b) => ("Segment", a, b),
            Def::Midpoint(a, b) => ("Midpoint", a, b),
            Def::Distance(a, b) => ("Distance", a, b),
            Def::CircleThrough { center, through } => ("Circle", center, through),
            Def::CircleRadius { center, radius } => ("Circle", center, radius),
            Def::IntersectLines(a, b) => ("Intersect", a, b),
            Def::Parallel { through, to } => ("Line", through, to),
            Def::Perpendicular { through, to } => ("PerpendicularLine", through, to),
            Def::Vector(VectorDef::Between(a, b)) => ("Vector", a, b),
            Def::Vector(VectorDef::Position(a)) => {
                return Some(self.command("Vector", &[print(a)]))
            }
            Def::Polygon(PolygonDef::Regular { a, b, n }) => {
                let n = match n {
                    Count::Literal(n) => self.number(n),
                    Count::Object(id) => print(id),
                };
                return Some(self.command("Polygon", &[print(a), print(b), n]));
            }
            Def::Polygon(PolygonDef::Polygon(ids)) => {
                let args = ctx
                    .c
                    .list(ids)
                    .iter()
                    .copied()
                    .map(print)
                    .collect::<Vec<_>>();
                return Some(self.command("Polygon", &args));
            }
            _ => return None,
        };
        Some(self.command(name, &[print(a), print(b)]))
    }

    fn list<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        run: Option<(u32, u32)>,
        depth: u32,
    ) -> Option<String> {
        let Some((start, len)) = run else {
            return Some("?".into());
        };
        let mut value = String::new();
        self.write_list(ctx, start, len, depth, &mut value)?;
        Some(value)
    }

    fn write_list<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        start: u32,
        len: u32,
        depth: u32,
        out: &mut String,
    ) -> Option<()> {
        if depth > expr::parse::MAX_DEPTH || ctx.stop(u64::from(len)) {
            return None;
        }
        let (left, sep, right) = if self.latex {
            (r"\left\{", r",\;", r"\right\}")
        } else {
            ("{", ", ", "}")
        };
        let lists = ctx.lists?;
        out.push_str(left);
        for i in 0..len {
            if i != 0 {
                out.push_str(sep);
            }
            let slot = lists.read(ctx.c, start + i);
            if let Some((s, n)) = slot.run() {
                self.write_list(ctx, s, n, depth + 1, out)?;
            } else {
                let text = if let Some(text) = slot.text_value() {
                    self.quoted(text)
                } else if let Some(id) = slot.object_id().filter(|_| slot.value.is_defined()) {
                    self.object_value(ctx, id)?
                } else if slot.kind == Kind::Text {
                    self.quoted("")
                } else {
                    self.shown(slot.kind, slot.shown())
                };
                if ctx.stop(text.len() as u64) {
                    return None;
                }
                self.append(out, &text)?;
            }
        }
        self.append(out, right)
    }

    fn append(self, out: &mut String, value: &str) -> Option<()> {
        if out.len().saturating_add(value.len()) > MAX_TEXT_BYTES {
            return None;
        }
        out.push_str(value);
        Some(())
    }

    fn quoted(self, text: &str) -> String {
        if self.latex {
            if !text.contains('/') && (text.contains(['\\', '_']) || text.contains("^{")) {
                format!("“{text}”")
            } else {
                format!(r"\text{{“{text}”}}")
            }
        } else {
            format!("\"{text}\"")
        }
    }
}

#[cfg(test)]
mod tests;
