//! The solving commands evaluated by Giac (the `giac` feature).
//!
//! The reference evaluates `Solve`, `Solutions`, `NSolve` and `NSolutions`
//! from the algebra input with `AlgoSolve`: it prints each argument's
//! definition, has its CAS translate the command with the `Ggb2giac`
//! templates and the Giac serialisation of expression trees
//! (`StringTemplate.giacTemplate`), evaluates that with Giac, and reads the
//! printed list back as construction objects (`evaluateToList`). This module
//! does the same with the kernel's trees and the Giac library loaded by
//! `geo-giac`; see docs/cas-giac.md.
//!
//! A translation that meets a tree it cannot print gives up (`None`), and the
//! caller keeps the kernel's own solver.

use super::{Op, Slot};
use crate::expr::eval::Ctx;
use crate::expr::{Bin, Entry, Lit, Node, Un};
use crate::{Construction, Def, Kind, Value};

/// The reference's `Ggb2giac` templates of the solving commands, with one
/// argument.
const SOLVE_1: &str = "ggbsort(normal([op(solve(%0))]))";
const SOLUTIONS_1: &str =
    "ggbsort(normal(solve(%0,when(type(%0)==DOM_LIST,lname(%0),when(contains(lname(%0),x),x,lname(%0)[0])),'!=')))";
const NSOLVE_1: &str = "ggbsort([[ggbnsans:=(%%0)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,[[ggbvars:=lname(ggbnsans)],[ggbnsans:=fsolve(ggbnsans,ggbvars)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,when(type(ggbnsans[0])==DOM_LIST,ggbnsans[0],ggbnsans),[ggbnsans])],seq(ggbvars[irem(j,dim(ggbnsans))]=ggbnsans[j],j,0,dim(ggbnsans)-1)][3],[[ggbvars:=lname(ggbnsans)],[ggbnsans:=fsolve(regroup(ggbnsans),ggbvars[0])],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,ggbnsans,[ggbnsans])],when(dim(ggbnsans)==0,{},seq(ggbvars[0]=ggbnsans[j],j,0,dim(ggbnsans)-1))][3])],ggbnsans][2])";
const NSOLUTIONS_1: &str = "ggbsort([[ggbnsans:=(%%0)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,[[ggbvars:=lname(ggbnsans)],[ggbnsans:=fsolve(ggbnsans,ggbvars)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,ggbnsans,[ggbnsans])],ggbnsans][3],[[ggbvars:=lname(ggbnsans)],[ggbnsans:=fsolve(regroup(ggbnsans),ggbvars[0])],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,ggbnsans,[ggbnsans])],ggbnsans][3])],ggbnsans][2])";

/// The two-argument templates.
const SOLVE_2: &str = "when(type(%0)==DOM_SYMBOLIC&&type(%1)==DOM_SYMBOLIC&&(%1)[0]!='of',(assume(%1),solve(%0,x)) [size(assume(%1),solve(%0,x))-1],when(type(%0)==DOM_IDENT&&type(%1)==DOM_SYMBOLIC&&%0=='x',(assume(%1),solve(%0=0,x))[size(assume(%1),solve(%0=0,x))-1],when(size(%1)==1,flatten1(ggbsort(normal([op(solve(%0,%1))]))),ggbsort(normal([op(solve(%0,%1))])))))";
const SOLUTIONS_2: &str = "ggbsort(normal(solve(%0,%1,'!=')))";
const NSOLVE_2: &str = "ggbsort([[ggbnsans:=(%%0)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,[[ggbvars:=seq(left(%%1[j]),j,0,dim(%%1)-1)],[ggbnsans:=fsolve(ggbnsans,%%1)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,when(type(ggbnsans[0])==DOM_LIST,ggbnsans[0],ggbnsans),[ggbnsans])],seq(ggbvars[irem(j,dim(ggbnsans))]=ggbnsans[j],j,0,dim(ggbnsans)-1)][3],[[ggbvars:=when(type(%%1)==DOM_LIST,left(%%1[0]),left(%%1))],[ggbnsans:=fsolve(ggbnsans,%%1)],[ggbnsans:=when(type(ggbnsans)==DOM_LIST,ggbnsans,[ggbnsans])],seq(ggbvars=ggbnsans[j],j,0,dim(ggbnsans)-1)][3])],ggbnsans][2])";
const NSOLUTIONS_2: &str = "ggbsort([[ggbnsans:=fsolve((%%0),%%1)],when(type(ggbnsans)==DOM_LIST,ggbnsans,[ggbnsans])][1])";

/// Evaluates a solving command through Giac. `None`: no translation, keep
/// the kernel's solver. `Some(None)`: an undefined list.
pub(super) fn solve<const M: bool>(
    ctx: &Ctx<'_, M>,
    equations: u32,
    op: Op,
) -> Option<Option<(u32, u32)>> {
    let Op::Solve {
        assignments,
        approximate,
        system,
        ..
    } = op
    else {
        return None;
    };
    let Node::List(start, length) = ctx.node(equations) else {
        return None;
    };
    // The command's node keeps the text of its second argument.
    let hint = ctx
        .e
        .nodes
        .iter()
        .enumerate()
        .skip(equations as usize)
        .find_map(|(k, node)| match node {
            Node::Cas(Op::Solve { .. }, a, _) if *a == equations => match &ctx.e.lits[k] {
                Lit::Text(text) => Some(Some(text.clone())),
                _ => Some(None),
            },
            _ => None,
        })?;
    let printer = Printer {
        c: ctx.c,
        names: hint
            .as_deref()
            .and_then(variable_names)
            .unwrap_or_else(|| vec!["x".into(), "y".into(), "z".into()]),
    };
    let mut items = Vec::with_capacity(length as usize);
    let mut variables = std::collections::BTreeSet::new();
    let mut is_list = false;
    for item in start..start + length {
        let Node::Item(i) = ctx.node(item) else {
            return None;
        };
        let mut out = String::new();
        printer.top(ctx.e, i, &mut out)?;
        is_list |= matches!(ctx.node(i), Node::Obj(id) if ctx.c.kind(id) == Kind::List);
        for name in ["x", "y", "z"] {
            if has_variable(&out, name) {
                variables.insert(name);
            }
        }
        items.push(out);
    }
    // `AlgoSolve.compute`: a list of equations is printed as a list, with
    // the sorted variables of its equations as the second argument.
    let argument = if system && !is_list {
        format!("{{{}}}", items.join(", "))
    } else if items.len() == 1 {
        items.pop()?
    } else {
        return None;
    };
    let second = match hint {
        Some(text) => Some(hint_argument(ctx.c, &text)?),
        None if system => Some(format!(
            "{{{}}}",
            variables.into_iter().collect::<Vec<_>>().join(", ")
        )),
        None => None,
    };
    let input = match second {
        Some(second) => {
            let template = match (assignments, approximate) {
                (true, false) => SOLVE_2,
                (false, false) => SOLUTIONS_2,
                (true, true) => NSOLVE_2,
                (false, true) => NSOLUTIONS_2,
            };
            fill(template, &[&argument, &second])
        }
        None => {
            let template = match (assignments, approximate) {
                (true, false) => SOLVE_1,
                (false, false) => SOLUTIONS_1,
                (true, true) => NSOLVE_1,
                (false, true) => NSOLUTIONS_1,
            };
            fill(template, &[&argument])
        }
    };
    let seed = geo_giac::session::seed(&input, 1);
    let raw = geo_giac::eval_raw(&input, geo_giac::session::DEFAULT_TIMEOUT_MS, seed).ok()?;
    if std::env::var_os("GEO_GIAC_TRACE").is_some() {
        eprintln!("giac input: {input}\ngiac output: {raw}");
    }
    let result = post_process(&raw);
    Some(to_slots(ctx, &result, system))
}

/// The names of a second argument that is a variable or a list of them,
/// as `parse_solve` reads them.
fn variable_names(hint: &str) -> Option<Vec<String>> {
    let inner = hint
        .strip_prefix('{')
        .and_then(|h| h.strip_suffix('}'))
        .unwrap_or(hint);
    let names = inner
        .split(',')
        .map(|n| n.trim().to_string())
        .collect::<Vec<_>>();
    let valid = |n: &String| {
        n.chars().next().is_some_and(char::is_alphabetic)
            && n.chars().all(|c| c.is_alphanumeric() || c == '_')
    };
    (names.len() <= 3 && names.iter().all(valid)).then_some(names)
}

/// Whether a printed equation names the variable.
fn has_variable(printed: &str, name: &str) -> bool {
    let bytes = printed.as_bytes();
    printed.match_indices(name).any(|(k, _)| {
        let before = k.checked_sub(1).map(|j| bytes[j]);
        let after = bytes.get(k + name.len()).copied();
        !before.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
    })
}

/// `AlgoSolve.printHint` and the Giac template for it: variables `x`, `y`,
/// `z` as they are, a number object as its value, any other name as the
/// CAS variable `ggbtmpvar<name>`, typed decimals as fractions.
fn hint_argument(c: &Construction, text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((k, ch)) = chars.next() {
        match ch {
            ' ' => {}
            '{' | '}' | '=' | '-' => out.push(ch),
            ',' => out.push_str(", "),
            c0 if c0.is_ascii_digit() || c0 == '.' => {
                let mut end = k + c0.len_utf8();
                while let Some(&(j, d)) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        end = j + d.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push_str(&decimal_fraction(&text[k..end]));
            }
            c0 if c0.is_alphabetic() => {
                let mut end = k + c0.len_utf8();
                while let Some(&(j, d)) = chars.peek() {
                    if d.is_alphanumeric() || d == '_' {
                        end = j + d.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let name = &text[k..end];
                match name {
                    "x" | "y" | "z" => out.push_str(name),
                    _ => match c.id(name).map(|id| c.value(id)) {
                        Some(Value::Number(v)) => number(v, &mut out),
                        Some(_) => return None,
                        None => {
                            out.push_str("ggbtmpvar");
                            out.push_str(name);
                        }
                    },
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

/// `GeoGebraCAS.getCASCommand`'s substitution: `%n` and `%%n` (numeric) are
/// replaced by argument `n`; the `_` escaping of `Ggb2giac.p` is undone.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let mut j = i + 1;
            if bytes.get(j) == Some(&b'%') {
                j += 1;
            }
            if let Some(d) = bytes.get(j).filter(|b| b.is_ascii_digit()) {
                if let Some(arg) = args.get(usize::from(d - b'0')) {
                    out.push_str(arg);
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Prints trees the way the reference's Giac template prints the
/// expression it parsed from an object's definition.
struct Printer<'a> {
    c: &'a Construction,
    /// The solving variables a command's own equations were parsed with
    /// ([`Node::CasVariable`] indexes them).
    names: Vec<String>,
}

impl Printer<'_> {
    /// An argument of a solving command: an equation `left=right` or an
    /// expression.
    fn top(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        match e.nodes[i as usize] {
            Node::Obj(id) => self.object(id, out),
            Node::Bin(Bin::Sub, l, r) if matches!(&e.lits[i as usize], Lit::Text(t) if &**t == "=") =>
            {
                self.expr(e, l, out)?;
                out.push('=');
                self.expr(e, r, out)
            }
            _ => self.expr(e, i, out),
        }
    }

    fn object(&self, id: crate::Id, out: &mut String) -> Option<()> {
        match self.c.def(id) {
            Def::Equation(_) | Def::ImplicitCurve(_) => {
                let left = crate::expr::equation::side_entry(self.c, id, true)?;
                let right = crate::expr::equation::side_entry(self.c, id, false)?;
                self.expr(left, left.root, out)?;
                out.push('=');
                self.expr(right, right.root, out)
            }
            _ if self.c.kind(id) == Kind::List => {
                let list = self.c.elements(id)?;
                out.push('{');
                for k in 0..list.len() {
                    if k > 0 {
                        out.push_str(", ");
                    }
                    self.object(list.object(k)?, out)?;
                }
                out.push('}');
                Some(())
            }
            _ => match self.c.value(id) {
                Value::Number(v) => {
                    number(v, out);
                    Some(())
                }
                _ => None,
            },
        }
    }

    /// Whether the printed definition of the subtree starts with a minus.
    fn leading_negative(&self, e: &Entry, i: u32) -> bool {
        match e.nodes[i as usize] {
            Node::Num(v) => v < 0.0,
            Node::Obj(id) => matches!(self.c.value(id), Value::Number(v) if v < 0.0),
            Node::Bin(Bin::Mul | Bin::Div, l, _) => {
                e.nodes[l as usize] != Node::MinusOne && self.leading_negative(e, l)
            }
            _ => false,
        }
    }

    /// The subtree with its leading number negated.
    fn negated(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        match e.nodes[i as usize] {
            Node::Num(v) => {
                match &e.lits[i as usize] {
                    Lit::Special(Some(dec)) => literal(&dec.negated(), out),
                    _ => number(-v, out),
                }
                Some(())
            }
            Node::Obj(id) => match self.c.value(id) {
                Value::Number(v) => {
                    number(-v, out);
                    Some(())
                }
                _ => None,
            },
            Node::Bin(op @ (Bin::Mul | Bin::Div), l, r) => {
                out.push('(');
                self.negated(e, l, out)?;
                out.push_str(if op == Bin::Mul { ")*" } else { ")/" });
                self.operand(e, r, out)
            }
            _ => None,
        }
    }

    fn operand(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        out.push('(');
        self.expr(e, i, out)?;
        out.push(')');
        Some(())
    }

    fn expr(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        match e.nodes[i as usize] {
            Node::Num(v) => {
                match &e.lits[i as usize] {
                    Lit::Special(Some(dec)) => literal(dec, out),
                    _ => number(v, out),
                }
                Some(())
            }
            Node::MinusOne => {
                out.push_str("-1");
                Some(())
            }
            Node::Var => {
                out.push('x');
                Some(())
            }
            Node::VarY => {
                out.push('y');
                Some(())
            }
            Node::CasVariable(n) => {
                let name = self.names.get(usize::from(n))?;
                if !matches!(name.as_str(), "x" | "y" | "z") {
                    out.push_str("ggbtmpvar");
                }
                out.push_str(name);
                Some(())
            }
            Node::Obj(id) => match self.c.value(id) {
                Value::Number(v) => {
                    number(v, out);
                    Some(())
                }
                _ => None,
            },
            Node::Bin(Bin::Mul, l, r) if e.nodes[l as usize] == Node::MinusOne => {
                out.push_str("-(");
                self.expr(e, r, out)?;
                out.push(')');
                Some(())
            }
            // `x + (-1) y` prints as `x - 1 y` in the definition the reference
            // reads back, and `x - (-1) y` as `x + 1 y`.
            Node::Bin(op @ (Bin::Add | Bin::Sub), l, r) if self.leading_negative(e, r) => {
                self.operand(e, l, out)?;
                out.push_str(if op == Bin::Add { "-" } else { "+" });
                out.push('(');
                self.negated(e, r, out)?;
                out.push(')');
                Some(())
            }
            Node::Bin(op, l, r) => {
                let sign = match op {
                    Bin::Add => "+",
                    Bin::Sub => "-",
                    Bin::Mul => "*",
                    Bin::Div => "/",
                    Bin::Pow => "^",
                    _ => return None,
                };
                self.operand(e, l, out)?;
                out.push_str(sign);
                self.operand(e, r, out)
            }
            Node::Un(op, a) => {
                let name = match op {
                    Un::Sqrt => "sqrt",
                    Un::Sin => "sin",
                    Un::Cos => "cos",
                    Un::Tan => "tan",
                    Un::Exp => "exp",
                    Un::Ln => "ln",
                    Un::Abs => "abs",
                    _ => return None,
                };
                out.push_str(name);
                self.operand(e, a, out)
            }
            _ => None,
        }
    }
}

/// A typed decimal (`MySpecialDouble`): an integer as written, otherwise
/// `(digits/10^k)`.
fn literal(dec: &crate::expr::decimal::Dec, out: &mut String) {
    match dec.signed_coefficient() {
        Some((coefficient, exp)) if (0..20).contains(&exp) => {
            let value = coefficient * 10i128.pow(exp as u32);
            out.push_str(&value.to_string());
        }
        Some((coefficient, exp)) if (-29..0).contains(&exp) => {
            let scale = 10i128.pow((-exp) as u32);
            let text = format!(
                "{}.{:0width$}",
                coefficient / scale,
                (coefficient % scale).abs(),
                width = (-exp) as usize
            );
            out.push_str(&decimal_fraction(&text));
        }
        _ => number(dec.to_f64(), out),
    }
}

/// A computed number (`MyDouble.toString` with the Giac template): `pi`,
/// `e`, an integer, or the 15-significant-digit decimal as an exact
/// fraction.
fn number(v: f64, out: &mut String) {
    if v == std::f64::consts::PI {
        out.push_str("pi");
        return;
    }
    if v == std::f64::consts::E {
        out.push('e');
        return;
    }
    if !v.is_finite() {
        out.push('?');
        return;
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        out.push_str(&format!("{}", v as i64));
        return;
    }
    // The definition is printed with 15 significant digits
    // (`StringTemplate.prefixedDefaultSF`), read back as a typed decimal and
    // printed by `convertScientificNotationGiac` after `canonicalNumber`.
    let digits = 14 - v.abs().log10().floor() as i32;
    if !(0..=40).contains(&digits) {
        out.push_str(&format!("{v}"));
        return;
    }
    let text = format!("{:.*}", digits as usize, v);
    out.push_str(&decimal_fraction(&text));
}

/// `convertScientificNotationGiac(canonicalNumber(text))` for a plain
/// decimal: `2.22` is `(222/100)`, `3.000` is `3`.
fn decimal_fraction(text: &str) -> String {
    let Some(dot) = text.find('.') else {
        return text.to_string();
    };
    if text[dot + 1..].bytes().all(|b| b == b'0') {
        return if dot == 0 {
            "0".into()
        } else {
            text[..dot].to_string()
        };
    }
    let text = if dot == 0 {
        format!("0{text}")
    } else {
        text.to_string()
    };
    let dot = text.find('.').unwrap_or(0);
    let joined = text.replace('.', "");
    let trimmed = joined.trim_start_matches('0');
    let numerator = if trimmed.is_empty() { "0" } else { trimmed };
    format!("({numerator}/1{})", "0".repeat(text.len() - dot - 1))
}

/// The parts of `CASgiac.postProcess` the solving commands meet.
fn post_process(s: &str) -> String {
    if s.contains("GIAC_ERROR") {
        return "?".into();
    }
    let ret = s.trim();
    if ret.contains("integrate(") {
        return "?".into();
    }
    ret.to_string()
}

/// A value printed by Giac.
#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Name(String),
    Neg(Box<Expr>),
    Bin(u8, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Eq(Box<Expr>, Box<Expr>),
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn peek(&mut self) -> Option<u8> {
        while self.s.get(self.i).is_some_and(|b| *b == b' ') {
            self.i += 1;
        }
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn equation(&mut self) -> Option<Expr> {
        let left = self.sum()?;
        if self.eat(b'=') {
            let right = self.sum()?;
            return Some(Expr::Eq(Box::new(left), Box::new(right)));
        }
        Some(left)
    }

    fn sum(&mut self) -> Option<Expr> {
        let mut left = self.product()?;
        loop {
            let op = match self.peek() {
                Some(b'+') => b'+',
                Some(b'-') => b'-',
                _ => return Some(left),
            };
            self.i += 1;
            let right = self.product()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn product(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(b'*') => b'*',
                Some(b'/') => b'/',
                _ => return Some(left),
            };
            self.i += 1;
            let right = self.unary()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.eat(b'-') {
            return Some(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat(b'+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<Expr> {
        // `√a`, which the reference's parser reads as `sqrt(a)`.
        let base = if self.peek() == Some(0xE2) && self.s[self.i..].starts_with("√".as_bytes()) {
            self.i += "√".len();
            Expr::Call("sqrt".into(), vec![self.atom()?])
        } else {
            self.atom()?
        };
        if self.eat(b'^') {
            let exponent = self.unary()?;
            return Some(Expr::Bin(b'^', Box::new(base), Box::new(exponent)));
        }
        Some(base)
    }

    fn items(&mut self, close: u8) -> Option<Vec<Expr>> {
        let mut items = Vec::new();
        if self.eat(close) {
            return Some(items);
        }
        loop {
            items.push(self.equation()?);
            if self.eat(close) {
                return Some(items);
            }
            if !self.eat(b',') {
                return None;
            }
        }
    }

    fn atom(&mut self) -> Option<Expr> {
        let b = self.peek()?;
        if b == b'{' || b == b'[' {
            self.i += 1;
            let close = if b == b'{' { b'}' } else { b']' };
            return Some(Expr::List(self.items(close)?));
        }
        if b == b'(' {
            self.i += 1;
            let mut items = self.items(b')')?;
            return Some(if items.len() == 1 {
                items.pop()?
            } else {
                Expr::Tuple(items)
            });
        }
        if b.is_ascii_digit() || b == b'.' {
            let start = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.i += 1;
            }
            if self.s.get(self.i).is_some_and(|c| *c == b'e' || *c == b'E')
                && self
                    .s
                    .get(self.i + 1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'-' || *c == b'+')
            {
                self.i += 2;
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
            }
            let text = std::str::from_utf8(&self.s[start..self.i]).ok()?;
            return text.parse().ok().map(Expr::Num);
        }
        if b.is_ascii_alphabetic() || b == b'_' || b == b'?' {
            let start = self.i;
            self.i += 1;
            while self
                .s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i])
                .ok()?
                .to_string();
            if self.eat(b'(') {
                return Some(Expr::Call(name, self.items(b')')?));
            }
            return Some(Expr::Name(name));
        }
        None
    }
}

fn parse(s: &str) -> Option<Expr> {
    let mut reader = Reader {
        s: s.as_bytes(),
        i: 0,
    };
    let e = reader.equation()?;
    (reader.peek().is_none()).then_some(e)
}

/// The value of a printed expression at `(x, y)`; `None` for anything the
/// reference would not read as a number.
fn value(e: &Expr, x: f64, y: f64) -> Option<f64> {
    Some(match e {
        Expr::Num(v) => *v,
        Expr::Name(n) => match n.as_str() {
            "x" => x,
            "y" => y,
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            _ => return None,
        },
        Expr::Neg(a) => -value(a, x, y)?,
        Expr::Bin(op, a, b) => {
            let (a, b) = (value(a, x, y)?, value(b, x, y)?);
            match op {
                b'+' => a + b,
                b'-' => a - b,
                b'*' => a * b,
                b'/' => a / b,
                _ => a.powf(b),
            }
        }
        Expr::Call(name, args) if args.len() == 1 => {
            let a = value(&args[0], x, y)?;
            match name.as_str() {
                "sqrt" => a.sqrt(),
                "sin" => a.sin(),
                "cos" => a.cos(),
                "tan" => a.tan(),
                "exp" => a.exp(),
                "ln" | "log" => a.ln(),
                "abs" => a.abs(),
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn has_name(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Name(n) => n == name,
        Expr::Num(_) => false,
        Expr::Neg(a) => has_name(a, name),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => has_name(a, name) || has_name(b, name),
        Expr::Call(_, args) | Expr::List(args) | Expr::Tuple(args) => {
            args.iter().any(|a| has_name(a, name))
        }
    }
}

/// One element of the list `AlgebraProcessor.evaluateToList` builds.
fn element<const M: bool>(ctx: &Ctx<'_, M>, e: &Expr) -> Option<Slot> {
    match e {
        Expr::List(items) => {
            let slots = items
                .iter()
                .map(|item| element(ctx, item))
                .collect::<Option<Vec<_>>>()?;
            let (start, len) = ctx.cas_slots(slots.into_iter())?;
            Some(Slot::list(start, len))
        }
        Expr::Eq(left, right) => {
            if has_name(e, "z") {
                return None;
            }
            let f = |x, y| Some(value(left, x, y)? - value(right, x, y)?);
            let c = f(0.0, 0.0)?;
            let a = f(1.0, 0.0)? - c;
            let b = f(0.0, 1.0)? - c;
            let check = f(2.0, 3.0)?;
            let scale = 1.0 + a.abs() + b.abs() + c.abs();
            if (check - (2.0 * a + 3.0 * b + c)).abs() > 1e-9 * scale {
                return None;
            }
            if a == 0.0 && b == 0.0 {
                // `x = x`: a line without coefficients, which is undefined.
                return Some(Slot::undefined(Kind::Line));
            }
            Some(Slot::line(Value::Line { a, b, c }))
        }
        Expr::Tuple(items) if items.len() == 2 => {
            let x = value(&items[0], 0.0, 0.0)?;
            let y = value(&items[1], 0.0, 0.0)?;
            if has_name(e, "x") || has_name(e, "y") {
                return None;
            }
            Some(Slot::point(Value::Point { x, y }))
        }
        // `{x}`: every number solves the equation; the reference reads the
        // function x.
        Expr::Name(name) if name == "x" => Some(Slot::polynomial(crate::cas::Poly::variable(0))),
        _ => {
            let (x, y) = (has_name(e, "x"), has_name(e, "y"));
            if x || y {
                // An expression in one free variable reads as a function of it.
                if x && y {
                    return None;
                }
                let variable = if x { "x" } else { "y" };
                let q = quotient(e, variable)?;
                let denominator = q.denominator.scalar()?;
                let one = crate::cas::Rat::ONE;
                let charge = |_| true;
                let work = &mut super::Work::new(&charge);
                let polynomial = q.numerator.scale(one.div(denominator)?, work)?;
                return Some(Slot::polynomial(polynomial));
            }
            Some(Slot::number(value(e, 0.0, 0.0)?))
        }
    }
}

/// An exact rational function of `variable` (the function's input).
fn quotient(e: &Expr, variable: &str) -> Option<crate::cas::Quotient> {
    use crate::cas::{Poly, Quotient, Rat};
    let charge = |_| true;
    let work = &mut super::Work::new(&charge);
    Some(match e {
        Expr::Num(v) => Quotient::polynomial(Poly::constant(Rat::from_value(*v)?)),
        Expr::Name(n) if n == variable => Quotient::polynomial(Poly::variable(0)),
        Expr::Neg(a) => {
            let zero = Quotient::polynomial(Poly::constant(Rat::new(0, 1)?));
            zero.sub(quotient(a, variable)?, work)?
        }
        Expr::Bin(op, a, b) => {
            let (a, b) = (quotient(a, variable)?, quotient(b, variable)?);
            match op {
                b'+' => a.add(b, work)?,
                b'-' => a.sub(b, work)?,
                b'*' => a.mul(b, work)?,
                b'/' => a.div(b, work)?,
                _ => {
                    let n = b.scalar()?;
                    let exponent = n.to_f64();
                    if exponent.fract() != 0.0 || exponent.abs() > 64.0 {
                        return None;
                    }
                    a.pow(exponent as i128, work)?
                }
            }
        }
        _ => return None,
    })
}

/// `AlgoSolve.compute` after `evaluateToList`: an undefined element makes
/// the list undefined, and the solutions of a list of equations that are not
/// lists themselves are wrapped in one more list.
fn to_slots<const M: bool>(ctx: &Ctx<'_, M>, result: &str, list_input: bool) -> Option<(u32, u32)> {
    let Expr::List(items) = parse(result)? else {
        return None;
    };
    let slots = items
        .iter()
        .map(|item| element(ctx, item))
        .collect::<Option<Vec<_>>>()?;
    if list_input && items.len() > 1 && !matches!(items[0], Expr::List(_)) {
        let (start, len) = ctx.cas_slots(slots.into_iter())?;
        return ctx.cas_slots(std::iter::once(Slot::list(start, len)));
    }
    ctx.cas_slots(slots.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_the_giac_template() {
        let print = |v| {
            let mut s = String::new();
            number(v, &mut s);
            s
        };
        assert_eq!(print(3.0), "3");
        assert_eq!(print(-1.0), "-1");
        assert_eq!(print(0.5), "(500000000000000/1000000000000000)");
        assert_eq!(print(-0.25), "(-0250000000000000/1000000000000000)");
        assert_eq!(print(-513.75), "(-513750000000000/1000000000000)");
    }

    #[test]
    fn templates_take_their_arguments() {
        assert_eq!(fill(SOLVE_1, &["x=3"]), "ggbsort(normal([op(solve(x=3))]))");
    }
}
