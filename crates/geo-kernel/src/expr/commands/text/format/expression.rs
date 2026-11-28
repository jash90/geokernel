// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionSerializer.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/StringTemplate.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{super::superscript, Format, MAX_TEXT_BYTES};
use crate::expr::commands::probability::Probability;
use crate::expr::eval::{Ctx, Env, Val};
use crate::expr::{self, Bin, Entry, Lit, Node, Un, NONE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Printed {
    Zero,
    Question,
    NegativeQuestion,
    Other,
}

impl Printed {
    fn of(text: &str) -> Self {
        match text {
            "0" => Self::Zero,
            "?" => Self::Question,
            "-?" => Self::NegativeQuestion,
            _ => Self::Other,
        }
    }
}

impl Format {
    /// AlgoDefined needs only to know whether the entire default value
    /// string is "?". Keep just the outcomes that its simplifications can
    /// expose, so large formulas need not be rendered.
    pub(super) fn has_value<const M: bool>(self, ctx: &Ctx<'_, M>, i: u32) -> Option<bool> {
        self.printed(ctx, i, expr::parse::MAX_DEPTH)
            .map(|printed| printed != Printed::Question)
    }

    fn printed<const M: bool>(self, ctx: &Ctx<'_, M>, i: u32, depth: u32) -> Option<Printed> {
        if depth == 0 {
            return Some(Printed::Other);
        }
        let node = ctx.node(i);
        match node {
            Node::Probability(op, _) if op.is_function() => return Some(Printed::Other),
            Node::Un(Un::X | Un::Y, _) if ctx.e.vars == 0 => {
                if ctx.stop(1) {
                    return None;
                }
                return Some(Printed::of(&self.number(ctx.full_number(i))));
            }
            Node::Apply(id, _) => {
                if ctx.stop(1) {
                    return None;
                }
                if let (crate::Def::Expr(def), crate::Value::Function) =
                    (ctx.c.def(id), ctx.c.value(id))
                {
                    let entry = expr::entry(ctx.c, def);
                    let nested = Ctx {
                        e: entry,
                        env: &Env::None,
                        ..*ctx
                    };
                    return self.printed(&nested, entry.root, depth - 1);
                }
                return Some(Printed::Other);
            }
            Node::Var
            | Node::VarY
            | Node::MinusOne
            | Node::Degree
            | Node::Bool(_)
            | Node::Tuple(..)
            | Node::Polar(..)
            | Node::Text
            | Node::If(..)
            | Node::List(..)
            | Node::Un(..)
            | Node::Apply2(..) => {
                return Some(Printed::Other);
            }
            _ => (),
        }
        if ctx.stop(1) {
            return None;
        }
        Some(match node {
            Node::Bin(op @ (Bin::Add | Bin::Sub), a, b) => {
                let right = self.printed(ctx, b, depth - 1)?;
                let left = self.printed(ctx, a, depth - 1)?;
                match (left, right) {
                    (_, Printed::Zero) => left,
                    (Printed::Zero, _) if op == Bin::Add => right,
                    _ => Printed::Other,
                }
            }
            // The degree suffix is printed before the factor rule, as in
            // `binary()`: 0° prints as 0°, not as the dropped factor 0.
            Node::Bin(Bin::Mul, _, b) if matches!(ctx.node(b), Node::Degree) => Printed::Other,
            Node::Bin(Bin::Mul, a, b) => match self.multiplier(ctx, a, b)? {
                Some("1") => self.printed(ctx, b, depth - 1)?,
                Some("-1") => match self.printed(ctx, b, depth - 1)? {
                    Printed::Question => Printed::NegativeQuestion,
                    Printed::NegativeQuestion if cancels_negative(ctx, b) => Printed::Question,
                    _ => Printed::Other,
                },
                Some("0") => Printed::Zero,
                _ => Printed::Other,
            },
            Node::Bin(Bin::FoldCount, a, b) => Printed::of(&self.number(expr::math::fold_count(
                ctx.full_number(a),
                ctx.full_number(b),
            ))),
            Node::Bin(..) => Printed::Other,
            Node::Item(a) => self.printed(ctx, a, depth - 1)?,
            _ => Printed::of(&self.expression(ctx, i, true, 0)?),
        })
    }

    fn multiplier<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        a: u32,
        b: u32,
    ) -> Option<Option<&'static str>> {
        let value = match ctx.node(a) {
            Node::Num(value) => value,
            Node::MinusOne => -1.0,
            Node::Obj(id) => match ctx.c.value(id) {
                crate::Value::Number(value) => value,
                _ => return Some(None),
            },
            _ => return Some(None),
        };
        Some(match self.number(value).as_str() {
            "1" => Some("1"),
            "-1" => Some("-1"),
            "0" | "-0"
                if value.abs() < crate::tol::MAX_PRECISION && polynomial_expression(ctx, b)? =>
            {
                Some("0")
            }
            _ => None,
        })
    }

    pub(super) fn source<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        src: &str,
        depth: u32,
    ) -> Option<String> {
        if ctx.stop(src.len() as u64) || depth > expr::parse::MAX_DEPTH {
            return None;
        }
        if let Ok(parsed) = expr::parse::parse(ctx.c, src) {
            let entry = Entry::new(ctx.c, parsed, false);
            let nested = Ctx {
                e: &entry,
                env: &Env::None,
                ..*ctx
            };
            return self.expression(&nested, entry.root, false, 0);
        }
        if let Some((name, args)) = crate::script::split_call(src) {
            let args = args
                .iter()
                .map(|arg| self.source(ctx, arg, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            return Some(self.command(name, &args));
        }
        Some(src.to_owned())
    }

    pub(super) fn expression<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        i: u32,
        substitute: bool,
        outer: u8,
    ) -> Option<String> {
        if ctx.stop(1) {
            return None;
        }
        let term = |a, precedence| self.expression(ctx, a, substitute, precedence);
        let text = match ctx.node(i) {
            Node::Num(x) if x == std::f64::consts::PI => {
                if self.latex {
                    r"\pi ".into()
                } else {
                    "π".into()
                }
            }
            Node::Num(x) if x == std::f64::consts::E => "ℯ".into(),
            Node::Num(x) => self.number(x),
            Node::MinusOne => "-1".into(),
            Node::Degree => {
                if self.latex {
                    r"^{\circ}".into()
                } else {
                    "°".into()
                }
            }
            Node::Bool(b) => b.to_string(),
            Node::Var => ctx
                .e
                .curve_parameter
                .as_deref()
                .unwrap_or(if ctx.e.function_of_y { "y" } else { "x" })
                .into(),
            Node::VarY => "y".into(),
            Node::Obj(id) | Node::FunctionObject(id) if !substitute => {
                Self::label(ctx.c.label(id), self.latex)
            }
            Node::Obj(id) | Node::FunctionObject(id) => self.object_value(ctx, id)?,
            Node::ItemObject(id) if substitute => self.object_value(ctx, id)?,
            Node::ItemObject(id) => Self::label(ctx.c.label(id), self.latex),
            Node::Tuple(x, y) => self.pair(&term(x, 0)?, &term(y, 0)?),
            Node::Polar(r, phi) => {
                self.parenthesize(format_args!("{}; {}", term(r, 0)?, term(phi, 0)?))
            }
            Node::Bin(op, a, b) => return self.binary(ctx, op, a, b, substitute, outer),
            Node::Un(op, a) => self.unary(op, term(a, 0)?),
            Node::Apply(id, a) => {
                self.command(&Self::label(ctx.c.label(id), self.latex), &[term(a, 0)?])
            }
            Node::Apply2(id, a, b) => self.command(
                &Self::label(ctx.c.label(id), self.latex),
                &[term(a, 0)?, term(b, 0)?],
            ),
            Node::If(_, cond, a, b) => {
                let mut args = vec![term(cond, 0)?, term(a, 0)?];
                if b != NONE {
                    args.push(term(b, 0)?);
                }
                self.command("If", &args)
            }
            Node::List(start, len) => {
                let (left, sep, right) = if self.latex {
                    (r"\left\{", r",\;", r"\right\}")
                } else {
                    ("{", ", ", "}")
                };
                let mut text = left.to_owned();
                for n in start..start + len {
                    if n != start {
                        self.append(&mut text, sep)?;
                    }
                    self.append(&mut text, &term(n, 0)?)?;
                }
                self.append(&mut text, right)?;
                text
            }
            Node::Text => {
                let Lit::Text(text) = &ctx.e.lits[i as usize] else {
                    return None;
                };
                self.quoted(text)
            }
            Node::Cmd(def, _) => match self.command_definition(ctx, def) {
                Some(command) => command,
                None => self.value(ctx, i)?,
            },
            Node::Probability(Probability::Exponential { function: true }, [_, rate, x, flag]) => {
                let x = term(x, 0)?;
                let rate = if substitute {
                    self.number(ctx.full_number(rate))
                } else {
                    term(rate, 6)?
                };
                let cumulative = flag == NONE || matches!(ctx.full(flag).val, Val::B(true, _));
                let product = if self.latex { r" \; " } else { " * " };
                let exponent = match rate.as_str() {
                    "1" => format!("-{x}"),
                    "-1" => x.clone(),
                    _ => format!("-{rate}{product}{x}"),
                };
                let survival = if self.latex {
                    format!(r"\mathit{{e}}^{{{exponent}}}")
                } else {
                    format!("exp({exponent})")
                };
                let branch = if cumulative {
                    format!("1 - {survival}")
                } else {
                    match rate.as_str() {
                        "1" => survival,
                        "-1" => format!("-{survival}"),
                        _ => format!("{rate}{product}{survival}"),
                    }
                };
                if self.latex {
                    format!(
                        r"\left\{{\begin{{array}}{{ll}} 0& : {x} < 0\\ {branch}& : \text{{otherwise}} \end{{array}}\right. "
                    )
                } else {
                    format!("If({x} < 0, 0, {branch})")
                }
            }
            Node::Probability(Probability::LogNormal { function: true }, [mean, sd, x, flag]) => {
                // The reference builds the expression from the live inputs,
                // so unsubstituted text keeps their names, not the command.
                let x = term(x, 0)?;
                let (mean, sd, base) = if substitute {
                    let mean = self.number(ctx.full_number(mean));
                    let sd = self.number(ctx.full_number(sd));
                    (mean, sd.clone(), sd)
                } else {
                    // The squared deviation needs power-level brackets, so
                    // `s + 1` is written as `(s + 1)²`.
                    (term(mean, 6)?, term(sd, 0)?, term(sd, 8)?)
                };
                // A negative number is not bracketed at power level.
                let base = if base.starts_with('-') {
                    self.parenthesize(&base)
                } else {
                    base
                };
                let log = if self.latex {
                    format!(r"\ln\left({x} \right)")
                } else {
                    format!("ln({x})")
                };
                let offset = if mean == "0" {
                    log
                } else if let Some(positive) = mean.strip_prefix('-') {
                    format!("{log} + {positive}")
                } else {
                    format!("{log} - {mean}")
                };
                let cumulative = flag == NONE || matches!(ctx.full(flag).val, Val::B(true, _));
                let branch = match (self.latex, cumulative) {
                    (true, true) => format!(
                        r"\operatorname{{erf}}\left(\frac{{{offset}}}{{\sqrt{{2}} \; \left|{sd}\right|}} \right) \cdot 0.5 + 0.5"
                    ),
                    (false, true) => format!("erf(({offset}) / (sqrt(2) abs({sd}))) * 0.5 + 0.5"),
                    (true, false) => format!(
                        r"\frac{{\mathit{{e}}^{{-\frac{{\left({offset} \right)^{{2}}}}{{{base}^{{2}} \cdot 2}}}}}}{{\left|{sd}\right| \; \sqrt{{2 \; \pi }} \; {x}}}"
                    ),
                    (false, false) => {
                        format!("ℯ^(-({offset})² / ({base}² * 2)) / (abs({sd}) sqrt(2π) {x})")
                    }
                };
                if self.latex {
                    format!(
                        r"\left\{{\begin{{array}}{{ll}} 0& : {x} ≤ 0\\ {branch}& : \text{{otherwise}} \end{{array}}\right. "
                    )
                } else {
                    format!("If({x} ≤ 0, 0, {branch})")
                }
            }
            Node::Probability(Probability::Weibull { function: true }, [shape, scale, x, flag]) => {
                // The reference builds the expression from the live inputs,
                // so unsubstituted LaTeX keeps their names, not the command.
                // Plain text shows the function's value string either way.
                let x = term(x, 0)?;
                let number = |i| self.number(ctx.full_number(i));
                let (shape, lowered, quotient, scale) = if substitute || !self.latex {
                    let (shape, scale) = (number(shape), number(scale));
                    (shape.clone(), shape.clone(), shape, scale)
                } else {
                    // LaTeX fractions need no parentheses around either part.
                    let operand = if self.latex { 0 } else { 6 };
                    let (quotient, scale) = (term(shape, operand)?, term(scale, operand)?);
                    (term(shape, 0)?, term(shape, 4)?, quotient, scale)
                };
                let cumulative = flag == NONE || matches!(ctx.full(flag).val, Val::B(true, _));
                let branch = if self.latex {
                    let ratio = format!(r"\left(\frac{{{x}}}{{{scale}}} \right)");
                    let power = if shape == "1" {
                        ratio.clone()
                    } else {
                        format!("{ratio}^{{{shape}}}")
                    };
                    let survival = format!(r"\mathit{{e}}^{{-{power}}}");
                    if cumulative {
                        format!("1 - {survival}")
                    } else {
                        format!(
                            r"\frac{{{quotient}}}{{{scale}}} \; {ratio}^{{{lowered} - 1}} \; {survival}"
                        )
                    }
                } else {
                    let ratio = format!("({x} / {scale})");
                    let power = match superscript(&shape) {
                        Some(digits) => format!("{ratio}{digits}"),
                        None => format!("{ratio}^({shape})"),
                    };
                    let survival = format!("ℯ^(-{power})");
                    if cumulative {
                        format!("1 - {survival}")
                    } else {
                        format!("{quotient} / {scale} {ratio}^({lowered} - 1) {survival}")
                    }
                };
                if self.latex {
                    format!(
                        r"\left\{{\begin{{array}}{{ll}} 0& : {x} < 0\\ {branch}& : \text{{otherwise}} \end{{array}}\right. "
                    )
                } else {
                    format!("If({x} < 0, 0, {branch})")
                }
            }
            Node::Probability(
                op @ (Probability::Normal { function: true }
                | Probability::Cauchy { function: true }
                | Probability::Logistic { function: true }),
                [mean, deviation, x, flag],
            ) => {
                let x = term(x, 0)?;
                let cauchy = matches!(op, Probability::Cauchy { .. });
                if !substitute {
                    let flag = if flag == NONE {
                        "true".into()
                    } else {
                        term(flag, 0)?
                    };
                    let name = match op {
                        Probability::Cauchy { .. } => "Cauchy",
                        Probability::Logistic { .. } => "Logistic",
                        _ => "Normal",
                    };
                    return Some(
                        self.command(name, &[term(mean, 0)?, term(deviation, 0)?, x, flag]),
                    );
                }
                let mean = self.number(ctx.full_number(mean));
                let sd = self.number(ctx.full_number(deviation));
                let offset = if mean == "0" {
                    x
                } else if let Some(positive) = mean.strip_prefix('-') {
                    format!("{x} + {positive}")
                } else {
                    format!("{x} - {mean}")
                };
                let cumulative = flag == NONE || matches!(ctx.full(flag).val, Val::B(true, _));
                if matches!(op, Probability::Logistic { .. }) {
                    let tail = if self.latex {
                        format!(r"\mathit{{e}}^{{-\frac{{{offset}}}{{\left|{sd}\right|}}}}")
                    } else {
                        format!("ℯ^(-({offset}) / abs({sd}))")
                    };
                    match (self.latex, cumulative) {
                        (true, true) => format!(r"\left({tail} + 1 \right)^{{-1}}"),
                        (false, true) => format!("({tail} + 1)⁻¹"),
                        (true, false) => format!(
                            r"\frac{{{tail}}}{{\left({tail} + 1 \right)^{{2}} \; \left|{sd}\right|}}"
                        ),
                        (false, false) => format!("{tail} / (({tail} + 1)² abs({sd}))"),
                    }
                } else if cauchy {
                    match (self.latex, cumulative) {
                        (true, true) => format!(
                            r"\frac{{\operatorname{{tan⁻¹}} \left( \frac{{{offset}}}{{\left|{sd}\right|}} \right)}}{{\pi }} + 0.5"
                        ),
                        (false, true) => format!("atan(({offset}) / abs({sd})) / π + 0.5"),
                        (_, false) => {
                            let base = if sd.starts_with('-') {
                                self.parenthesize(&sd)
                            } else {
                                sd.clone()
                            };
                            if self.latex {
                                format!(
                                    r"\frac{{\left|{sd}\right|}}{{\left({base}^{{2}} + \left({offset} \right)^{{2}} \right) \; \pi }}"
                                )
                            } else {
                                format!("abs({sd}) / (({base}² + ({offset})²) * π)")
                            }
                        }
                    }
                } else if cumulative {
                    if self.latex {
                        format!(
                            r"\frac{{\operatorname{{erf}}\left(\frac{{{offset}}}{{\left|{sd}\right| \; \sqrt{{2}}}} \right) + 1}}{{2}}"
                        )
                    } else {
                        format!("(erf(({offset}) / (abs({sd}) sqrt(2))) + 1) / 2")
                    }
                } else {
                    let base = if sd.starts_with('-') {
                        self.parenthesize(&sd)
                    } else {
                        sd.clone()
                    };
                    if self.latex {
                        format!(
                            r"\frac{{\mathit{{e}}^{{\frac{{-\left({offset} \right)^{{2}}}}{{{base}^{{2}} \cdot 2}}}}}}{{\left|{sd}\right| \; \sqrt{{2 \; \pi }}}}"
                        )
                    } else {
                        format!("exp(-({offset})² / ({base}² * 2)) / (abs({sd}) sqrt(2 π))")
                    }
                }
            }
            Node::Item(a) => return term(a, outer),
            _ => self.value(ctx, i)?,
        };
        Some(text)
    }

    fn binary<const M: bool>(
        self,
        ctx: &Ctx<'_, M>,
        op: Bin,
        a: u32,
        b: u32,
        substitute: bool,
        outer: u8,
    ) -> Option<String> {
        let term = |i, precedence| self.expression(ctx, i, substitute, precedence);
        let (precedence, expr) = match op {
            Bin::FoldCount => {
                let count = crate::expr::math::fold_count(ctx.full_number(a), ctx.full_number(b));
                (8, self.number(count))
            }
            Bin::Mul if matches!(ctx.node(b), Node::Degree) => {
                (8, format!("{}{}", term(a, 8)?, term(b, 0)?))
            }
            Bin::Mul if matches!(ctx.node(a), Node::MinusOne) => (6, format!("-{}", term(b, 6)?)),
            Bin::Div if self.latex => (8, format!(r"\frac{{{}}}{{{}}}", term(a, 0)?, term(b, 0)?)),
            Bin::Pow | Bin::RootPow => {
                let x = term(a, 8)?;
                let exponent = term(b, 0)?;
                let power = if self.latex {
                    format!("^{{{exponent}}}")
                } else if let Some(digits) = superscript(&exponent) {
                    digits
                } else {
                    format!("^({exponent})")
                };
                (7, format!("{x}{power}"))
            }
            Bin::Mul => {
                let base = match ctx.node(b) {
                    Node::Bin(Bin::Pow | Bin::RootPow, base, _) => ctx.node(base),
                    node => node,
                };
                let variable = matches!(base, Node::Var | Node::VarY);
                if substitute && self.simplify_coefficients {
                    match self.multiplier(ctx, a, b)? {
                        Some("1") => return term(b, outer),
                        Some("-1") => return Some(format!("-{}", term(b, 6)?)),
                        Some("0") => return Some("0".into()),
                        _ => (),
                    }
                }
                let numeric = matches!(ctx.node(a), Node::Num(x) if x.is_finite()
                    && x != std::f64::consts::PI && x != std::f64::consts::E);
                let implicit =
                    variable || !substitute && matches!(ctx.node(b), Node::Obj(_)) && numeric;
                let sep = match (self.latex, implicit) {
                    (false, false) => " * ",
                    (false, true) if variable && numeric => "",
                    (false, true) => " ",
                    (true, false) => r" \cdot ",
                    (true, true) => r" \; ",
                };
                (5, format!("{}{sep}{}", term(a, 5)?, term(b, 6)?))
            }
            Bin::Add | Bin::Sub => {
                let zero = |i| match ctx.node(i) {
                    Node::Num(x) => x == 0.0,
                    Node::Obj(id) => ctx.c.value(id) == crate::Value::Number(0.0),
                    _ => false,
                };
                if substitute {
                    if zero(b) {
                        return term(a, outer);
                    }
                    if op == Bin::Add && zero(a) {
                        return term(b, outer);
                    }
                }
                let right = term(b, 5)?;
                if substitute && self.simplify_coefficients && right == "0" {
                    return term(a, outer);
                }
                let left = term(a, 4)?;
                if substitute && self.simplify_coefficients && op == Bin::Add && left == "0" {
                    return Some(right);
                }
                let (negative, term) = right
                    .strip_prefix('-')
                    .map_or((false, right.as_str()), |rest| (true, rest));
                let sign = if (op == Bin::Sub) ^ negative {
                    " - "
                } else {
                    " + "
                };
                (4, format!("{left}{sign}{term}"))
            }
            Bin::Div => (5, format!("{} / {}", term(a, 6)?, term(b, 6)?)),
            Bin::Less
            | Bin::Greater
            | Bin::LessEqual
            | Bin::GreaterEqual
            | Bin::Equal
            | Bin::NotEqual => {
                let sign = comparison_sign(op);
                (3, format!("{}{sign}{}", term(a, 4)?, term(b, 4)?))
            }
            _ => return Some(self.command(binary_name(op), &[term(a, 0)?, term(b, 0)?])),
        };
        if expr.len() > MAX_TEXT_BYTES {
            return None;
        }
        Some(if precedence < outer {
            self.parenthesize(expr)
        } else {
            expr
        })
    }

    fn unary(self, op: Un, arg: String) -> String {
        match (self.latex, op) {
            (true, Un::Sqrt) => format!(r"\sqrt{{{arg}}}"),
            (true, Un::Cbrt) => format!(r"\sqrt[3]{{{arg}}}"),
            (true, Un::Abs) => format!(r"\left|{arg} \right|"),
            (_, Un::Factorial) => format!("{arg}!"),
            (false, Un::Abs) => format!("abs({arg})"),
            (true, Un::X | Un::Y) | (false, _) => self.command(unary_name(op), &[arg]),
            (true, _) => format!(r"\operatorname{{{}}} \left( {arg} \right)", unary_name(op)),
        }
    }
}

// StringTemplate strips a printed minus when the right operand's
// first multiplier is exactly -1.
fn cancels_negative<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> bool {
    matches!(ctx.node(i), Node::Bin(Bin::Mul, a, _) if match ctx.node(a) {
        Node::MinusOne | Node::Num(-1.0) => true,
        Node::Obj(id) => ctx.c.value(id) == crate::Value::Number(-1.0),
        _ => false,
    })
}

/// StringTemplate accepts constant subexpressions and leaves, and checks
/// the operations that combine variables separately.
fn polynomial_expression<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> Option<bool> {
    if ctx.stop(1) {
        return None;
    }
    Some(match ctx.node(i) {
        Node::Bin(Bin::Add | Bin::Sub | Bin::Mul, a, b) => {
            polynomial_expression(ctx, a)? && polynomial_expression(ctx, b)?
        }
        Node::Bin(Bin::Div | Bin::Pow, a, b) => {
            polynomial_expression(ctx, a)? && constant_expression(ctx, b)?
        }
        Node::Apply(..) | Node::Apply2(..) => false,
        Node::Bin(..) | Node::Un(..) | Node::If(..) => constant_expression(ctx, i)?,
        _ => true,
    })
}

fn constant_expression<const M: bool>(ctx: &Ctx<'_, M>, i: u32) -> Option<bool> {
    if ctx.stop(1) {
        return None;
    }
    Some(match ctx.node(i) {
        Node::Num(_) | Node::MinusOne | Node::Degree | Node::Bool(_) => true,
        Node::Bin(_, a, b) => constant_expression(ctx, a)? && constant_expression(ctx, b)?,
        Node::Un(_, a) => constant_expression(ctx, a)?,
        Node::If(crate::expr::IfMode::Algo | crate::expr::IfMode::AlgoNext, test, yes, no) => {
            constant_expression(ctx, test)?
                && constant_expression(ctx, yes)?
                && (no == NONE || constant_expression(ctx, no)?)
        }
        _ => false,
    })
}

fn comparison_sign(op: Bin) -> &'static str {
    match op {
        Bin::Less => " < ",
        Bin::Greater => " > ",
        Bin::LessEqual => " ≤ ",
        Bin::GreaterEqual => " ≥ ",
        Bin::Equal => " ≟ ",
        _ => " ≠ ",
    }
}

fn unary_name(op: Un) -> &'static str {
    match op {
        Un::Factorial => "factorial",
        Un::Sin => "sin",
        Un::Cos => "cos",
        Un::Tan => "tan",
        Un::Csc => "csc",
        Un::Sec => "sec",
        Un::Cot => "cot",
        Un::Sinh => "sinh",
        Un::Cosh => "cosh",
        Un::Tanh => "tanh",
        Un::Csch => "csch",
        Un::Sech => "sech",
        Un::Coth => "coth",
        Un::Asin | Un::AsinD => "asin",
        Un::Acos | Un::AcosD => "acos",
        Un::Atan | Un::AtanD => "atan",
        Un::Asinh => "asinh",
        Un::Acosh => "acosh",
        Un::Atanh => "atanh",
        Un::Sqrt => "sqrt",
        Un::Cbrt => "cbrt",
        Un::Abs => "abs",
        Un::Length => "Length",
        Un::Sgn => "sgn",
        Un::Floor => "floor",
        Un::Ceil => "ceil",
        Un::Round => "round",
        Un::Exp => "exp",
        Un::Ln => "ln",
        Un::Log10 => "lg",
        Un::Log2 => "ld",
        Un::Gamma => "gamma",
        Un::X => "x",
        Un::Y => "y",
        Un::Not => "¬",
    }
}

fn binary_name(op: Bin) -> &'static str {
    match op {
        Bin::Atan2 => "atan2",
        Bin::LogB => "log",
        Bin::Round2 => "round",
        Bin::Mod => "Mod",
        Bin::IntDiv => "Div",
        Bin::Gcd => "GCD",
        Bin::Lcm => "LCM",
        Bin::CommonDenominator => "CommonDenominator",
        Bin::Max => "Max",
        Bin::Min => "Min",
        Bin::And => "And",
        Bin::Or => "Or",
        Bin::Xor => "Xor",
        Bin::Implies => "Implies",
        _ => unreachable!("operator has an infix or special form"),
    }
}
