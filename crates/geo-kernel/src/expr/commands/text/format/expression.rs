// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/arithmetic/ExpressionSerializer.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/StringTemplate.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

use super::{Format, MAX_TEXT_BYTES};
use crate::expr::eval::{Ctx, Env};
use crate::expr::{self, Bin, Entry, Lit, Node, Un, NONE};

impl Format {
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
            Node::Var => "x".into(),
            Node::VarY => "y".into(),
            Node::Obj(id) | Node::FunctionObject(id) if !substitute => {
                Self::label(ctx.c.label(id), self.latex)
            }
            Node::Obj(id) => self.shown(ctx.c.kind(id), ctx.c.value(id)),
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
            Bin::Mul if matches!(ctx.node(b), Node::Degree) => {
                (8, format!("{}{}", term(a, 8)?, term(b, 0)?))
            }
            Bin::Mul if matches!(ctx.node(a), Node::MinusOne) => (6, format!("-{}", term(b, 6)?)),
            Bin::Div if self.latex => (8, format!(r"\frac{{{}}}{{{}}}", term(a, 0)?, term(b, 0)?)),
            Bin::Pow => {
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
                let implicit = matches!(ctx.node(b), Node::Var | Node::VarY)
                    || !substitute
                        && matches!(ctx.node(b), Node::Obj(_))
                        && matches!(ctx.node(a), Node::Num(_));
                let sep = match (self.latex, implicit) {
                    (false, false) => " * ",
                    (false, true) => " ",
                    (true, false) => r" \cdot ",
                    (true, true) => r" \; ",
                };
                (5, format!("{}{sep}{}", term(a, 5)?, term(b, 6)?))
            }
            Bin::Add | Bin::Sub => {
                let sign = if op == Bin::Add { " + " } else { " - " };
                (4, format!("{}{sign}{}", term(a, 4)?, term(b, 5)?))
            }
            Bin::Div => (5, format!("{} / {}", term(a, 6)?, term(b, 6)?)),
            Bin::Less
            | Bin::Greater
            | Bin::LessEqual
            | Bin::GreaterEqual
            | Bin::Equal
            | Bin::NotEqual => {
                let sign = match op {
                    Bin::Less => " < ",
                    Bin::Greater => " > ",
                    Bin::LessEqual => " ≤ ",
                    Bin::GreaterEqual => " ≥ ",
                    Bin::Equal => " ≟ ",
                    _ => " ≠ ",
                };
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

fn superscript(s: &str) -> Option<String> {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    s.chars()
        .map(|c| match c {
            '-' => Some('⁻'),
            c => Some(DIGITS[c.to_digit(10)? as usize]),
        })
        .collect()
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
