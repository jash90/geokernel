//! The algebra commands evaluated by Giac (the `giac` feature).
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

pub(super) mod rewrite;

use super::{syntax, Op, Slot};
use syntax::Expr;

fn parse(s: &str) -> Option<Expr> {
    syntax::parse(s, false)
}
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

/// `PlotSolve.1`: the reference's `PointList.1` around `Solve.1`.
const PLOTSOLVE_1: &str = "[[ggbplarg:=ggbsort(normal([op(solve(%0))]))],[when(size((ggbplarg)[0])==3, flatten1(coordinates(map(ggbplarg,t->when((flatten1(t))[0][0]==equal&&type((flatten1(t))[0][2])=='DOM_INT',point((flatten1(t))[0][2],(flatten1(t))[1][2],(flatten1(t))[2][2]),when(type(flatten1(t)[0])==DOM_INT,point(t[0],t[1],t[2]),point((flatten1(t))[0][2],(flatten1(t))[1][2],(flatten1(t))[2][2])))))),when(type((ggbplarg)[0])==DOM_COMPLEX||(type((ggbplarg)[0])==DOM_SYMBOLIC&&(ggbplarg)[0][0]=='*'&& (ggbplarg)[0][1]==i),ggbplarg,flatten1(coordinates(map(ggbplarg,t->when(t[0]==equal,point(re(t[2]),im(t[2])),t))))))]][-1][0]";

/// A command without a template of its arity is passed to Giac as an unknown
/// function (`GeoGebraCAS.getCASCommand`).
const PLOTSOLVE_2: &str = "ggbtmpvarPlotSolve(%0,%1)";

pub(super) fn row_solve(name: &str, input: &str, variable: &str) -> String {
    let template = match name {
        "Solve" => SOLVE_2,
        "Solutions" => SOLUTIONS_2,
        "NSolve" => NSOLVE_2,
        _ => NSOLUTIONS_2,
    };
    fill(template, &[input, variable], &[input, variable])
}

pub(super) fn row_object(c: &Construction, id: crate::Id) -> Option<String> {
    let mut result = String::new();
    let mut printer = Printer::new(c, Vec::new(), false);
    printer.row = true;
    printer.object(id, &mut result)?;
    Some(result)
}

/// Evaluates a solving command through Giac. `None`: no translation, keep
/// the kernel's solver. `Some(None)`: an undefined list.
pub(super) fn solve<const M: bool>(
    ctx: &Ctx<'_, M>,
    equations: u32,
    op: Op,
) -> Option<Option<(u32, u32)>> {
    // The backend is chosen once per process (docs/cas-giac.md, "Backends
    // and failures").
    match geo_giac::backend() {
        geo_giac::Backend::KernelOnly => return None,
        geo_giac::Backend::Failed => return Some(None),
        geo_giac::Backend::Giac => {}
    }
    match translate_and_evaluate(ctx, equations, op) {
        Evaluation::Done(result) => Some(result),
        // A Giac call that fails is an undefined result, as a failed CAS
        // call is in the reference: never the kernel's own answer.
        Evaluation::Failed => Some(None),
        // No translation: the kernel's solver answers, unless the command
        // has a form only the Giac backend accepts, which it would answer
        // with that form ignored.
        Evaluation::Untranslated if giac_only(ctx, equations, op) => Some(None),
        Evaluation::Untranslated => None,
    }
}

enum Evaluation {
    Done(Option<(u32, u32)>),
    Failed,
    Untranslated,
}

/// The text of a solving command's second argument, kept on its node.
fn hint_of<const M: bool>(ctx: &Ctx<'_, M>, equations: u32) -> Option<Option<std::sync::Arc<str>>> {
    ctx.e
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
        })
}

/// Whether the command is `PlotSolve` or has a second argument that is not
/// a list of variables, which only the Giac backend accepts.
fn giac_only<const M: bool>(ctx: &Ctx<'_, M>, equations: u32, op: Op) -> bool {
    let plot = matches!(op, Op::Solve { plot: true, .. });
    let hint = hint_of(ctx, equations).flatten();
    plot || hint.is_some_and(|h| variable_names(&h).is_none())
}

fn translate_and_evaluate<const M: bool>(ctx: &Ctx<'_, M>, equations: u32, op: Op) -> Evaluation {
    match translate(ctx, equations, op) {
        Some((input, system)) => match evaluate_raw(&input) {
            Some(result) => Evaluation::Done(to_slots(ctx, &result, system)),
            None => Evaluation::Failed,
        },
        None => Evaluation::Untranslated,
    }
}

pub(super) fn evaluate_raw(input: &str) -> Option<String> {
    let seed = geo_giac::session::seed(input, 1);
    let raw = geo_giac::eval_raw(input, geo_giac::session::DEFAULT_TIMEOUT_MS, seed).ok()?;
    if std::env::var_os("GEO_GIAC_TRACE").is_some() {
        eprintln!("giac input: {input}\ngiac output: {raw}");
    }
    Some(post_process(&raw))
}

/// The Giac input of a solving command, and whether its argument is a list.
fn translate<const M: bool>(ctx: &Ctx<'_, M>, equations: u32, op: Op) -> Option<(String, bool)> {
    let Op::Solve {
        assignments,
        approximate,
        system,
        plot,
        ..
    } = op
    else {
        return None;
    };
    let Node::List(start, length) = ctx.node(equations) else {
        return None;
    };
    // The command's node keeps the text of its second argument.
    let hint = hint_of(ctx, equations)?;
    let names = with_cartesian_variables(
        hint.as_deref()
            .and_then(|h| variable_names(h).or_else(|| start_value_names(h)))
            .unwrap_or_default(),
    );
    let printer = Printer::new(ctx.c, names.clone(), false);
    let numeric_printer = Printer::new(ctx.c, names, true);
    let mut items = Vec::with_capacity(length as usize);
    let mut numeric_items = Vec::with_capacity(length as usize);
    let mut variables = std::collections::BTreeSet::new();
    let mut is_list = false;
    for item in start..start + length {
        let Node::Item(i) = ctx.node(item) else {
            return None;
        };
        let mut out = String::new();
        printer.top(ctx.e, i, &mut out)?;
        let mut numeric = String::new();
        numeric_printer.top(ctx.e, i, &mut numeric)?;
        numeric_items.push(numeric);
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
    let (argument, numeric_argument) = if system && !is_list {
        (
            format!("{{{}}}", items.join(", ")),
            format!("{{{}}}", numeric_items.join(", ")),
        )
    } else if items.len() == 1 {
        (items.pop()?, numeric_items.pop()?)
    } else {
        return None;
    };
    let second = match &hint {
        Some(text) => Some((
            hint_argument(ctx.c, text, false)?,
            hint_argument(ctx.c, text, true)?,
        )),
        None if system => {
            let list = format!(
                "{{{}}}",
                variables.into_iter().collect::<Vec<_>>().join(", ")
            );
            Some((list.clone(), list))
        }
        None => None,
    };
    let input = match second {
        Some(second) => {
            let template = match (assignments, approximate) {
                _ if plot => PLOTSOLVE_2,
                (true, false) => SOLVE_2,
                (false, false) => SOLUTIONS_2,
                (true, true) => NSOLVE_2,
                (false, true) => NSOLUTIONS_2,
            };
            fill(
                template,
                &[&argument, &second.0],
                &[&numeric_argument, &second.1],
            )
        }
        None => {
            let template = match (assignments, approximate) {
                _ if plot => PLOTSOLVE_1,
                (true, false) => SOLVE_1,
                (false, false) => SOLUTIONS_1,
                (true, true) => NSOLVE_1,
                (false, true) => NSOLUTIONS_1,
            };
            fill(template, &[&argument], &[&numeric_argument])
        }
    };
    Some((input, system))
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
    (names.len() <= 3 && names.iter().all(|n| is_name(n))).then_some(names)
}

fn is_name(n: &str) -> bool {
    n.chars().next().is_some_and(char::is_alphabetic)
        && n.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// The solving variables of a second argument that is not a list of
/// variables (`t = 1`, `{x = 1, y = 2}`): the names left of each `=`, then
/// `x`, `y`, `z`. `None` when it names none.
pub(crate) fn start_value_names(hint: &str) -> Option<Vec<String>> {
    let inner = hint
        .strip_prefix('{')
        .and_then(|h| h.strip_suffix('}'))
        .unwrap_or(hint);
    let mut names: Vec<String> = Vec::new();
    for item in inner.split(',') {
        let (name, _) = item.split_once('=')?;
        let name = name.trim();
        if !is_name(name) {
            return None;
        }
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    Some(with_cartesian_variables(names))
}

pub(in crate::expr) fn with_cartesian_variables(mut names: Vec<String>) -> Vec<String> {
    for name in ["x", "y", "z"] {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
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

/// `AlgoSolve.printHint` and the Giac template for it: the second argument
/// read as an expression and printed with bracketed operands; variables
/// `x`, `y`, `z` as they are, a number object as its value, any other name
/// as the CAS variable `ggbtmpvar<name>`, decimals as fractions (or as
/// decimals in the numeric template).
fn hint_argument(c: &Construction, text: &str, numeric: bool) -> Option<String> {
    let e = parse(text)?;
    let mut out = String::new();
    print_hint(c, &e, numeric, true, &mut out)?;
    Some(out)
}

fn print_hint(
    c: &Construction,
    e: &Expr,
    numeric: bool,
    top: bool,
    out: &mut String,
) -> Option<()> {
    let operand = |e: &Expr, out: &mut String| -> Option<()> {
        out.push('(');
        print_hint(c, e, numeric, false, out)?;
        out.push(')');
        Some(())
    };
    match e {
        Expr::Num(v, _) => {
            let text = format!("{v}");
            if numeric {
                out.push_str(&canonical_number(&text));
            } else {
                out.push_str(&decimal_fraction(&text));
            }
        }
        Expr::Name(name) => match name.as_str() {
            "x" | "y" | "z" => out.push_str(name),
            _ => match c.id(name).map(|id| c.value(id)) {
                Some(Value::Number(v)) if numeric => numeric_number(v, out),
                Some(Value::Number(v)) => number(v, out),
                Some(Value::Point { x, y }) => {
                    out.push_str("point(");
                    if numeric {
                        numeric_number(x, out);
                        out.push(',');
                        numeric_number(y, out);
                    } else {
                        number(x, out);
                        out.push(',');
                        number(y, out);
                    }
                    out.push(')');
                }
                Some(_) => return None,
                None => {
                    out.push_str("ggbtmpvar");
                    out.push_str(name);
                }
            },
        },
        Expr::Neg(a) => match &**a {
            // A typed literal is negated in place.
            Expr::Num(..) => {
                out.push('-');
                print_hint(c, a, numeric, false, out)?;
            }
            _ => {
                out.push_str("-(");
                print_hint(c, a, numeric, false, out)?;
                out.push(')');
            }
        },
        Expr::Bin(op, a, b) => {
            operand(a, out)?;
            out.push(*op as char);
            operand(b, out)?;
        }
        Expr::Eq(a, b) if top => {
            if matches!(&**a, Expr::Name(name) if name == "z") {
                return print_hint(c, b, numeric, false, out);
            }
            print_hint(c, a, numeric, false, out)?;
            out.push('=');
            print_hint(c, b, numeric, false, out)?;
        }
        Expr::List(items) if top => {
            out.push('{');
            for (k, item) in items.iter().enumerate() {
                if k > 0 {
                    out.push_str(", ");
                }
                print_hint(c, item, numeric, true, out)?;
            }
            out.push('}');
        }
        _ => return None,
    }
    Some(())
}

/// `GeoGebraCAS.getCASCommand`'s substitution: `%n` and `%%n` (numeric) are
/// replaced by argument `n`; the `_` escaping of `Ggb2giac.p` is undone.
fn fill(template: &str, args: &[&str], numeric: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let mut j = i + 1;
            let numeric_form = bytes.get(j) == Some(&b'%');
            if numeric_form {
                j += 1;
            }
            let args = if numeric_form { numeric } else { args };
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
    /// The numeric Giac template (`%%n` in a template): decimals as
    /// decimals, not fractions.
    numeric: bool,
    parameter: Option<&'a str>,
    row: bool,
    remaining: std::cell::Cell<usize>,
    depth: std::cell::Cell<usize>,
}

impl<'a> Printer<'a> {
    fn new(c: &'a Construction, names: Vec<String>, numeric: bool) -> Self {
        Self {
            c,
            names,
            numeric,
            parameter: None,
            row: false,
            remaining: std::cell::Cell::new(8192),
            depth: std::cell::Cell::new(0),
        }
    }

    fn bounded(&self, print: impl FnOnce() -> Option<()>) -> Option<()> {
        let remaining = self.remaining.get().checked_sub(1)?;
        let depth = self.depth.get();
        if depth >= 128 {
            return None;
        }
        self.remaining.set(remaining);
        self.depth.set(depth + 1);
        let result = print();
        self.depth.set(depth);
        result
    }

    /// An argument of a solving command: an equation `left=right` or an
    /// expression.
    fn top(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        match e.nodes[i as usize] {
            Node::Obj(id) | Node::FunctionObject(id) => self.object(id, out),
            Node::Bin(Bin::Sub, l, r) if matches!(&e.lits[i as usize], Lit::Text(t) if &**t == "=") =>
            {
                if matches!(e.nodes[l as usize], Node::CasVariable(n) if self.names.get(usize::from(n)).is_some_and(|name| name == "z"))
                    && matches!(e.nodes[r as usize], Node::Num(_) | Node::Obj(_))
                {
                    return self.expr(e, r, out);
                }
                self.expr(e, l, out)?;
                out.push('=');
                self.expr(e, r, out)
            }
            _ => self.expr(e, i, out),
        }
    }

    fn object(&self, id: crate::Id, out: &mut String) -> Option<()> {
        self.bounded(|| self.object_value(id, out))
    }

    fn object_value(&self, id: crate::Id, out: &mut String) -> Option<()> {
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
            Def::Expr(def)
                if self.c.kind(id) == Kind::Function
                    || self.parameter.is_some() && self.c.kind(id) == Kind::FunctionNVar =>
            {
                // `FunctionExpander`: a function prints as its expression.
                let entry = crate::expr::entry(self.c, def);
                if entry.function_of_y
                    || entry.curve_parameter.is_some() && self.parameter.is_none()
                {
                    return None;
                }
                let entry = match &entry.extra {
                    crate::expr::StoredExtra::Cas(form) => form.tree().unwrap_or(entry),
                    _ => entry,
                };
                self.expr(entry, entry.root, out)
            }
            _ => match self.c.value(id) {
                Value::Number(v) => {
                    if self.row {
                        if let Some(definition) = super::input::number_definition(self.c, id) {
                            let before = out.len();
                            if self.expr(&definition, definition.root, out).is_some() {
                                return Some(());
                            }
                            out.truncate(before);
                        }
                        if let Some(decimal) = crate::expr::free_decimal(self.c, id) {
                            self.literal(decimal, out);
                            return Some(());
                        }
                    }
                    self.object_number(id, v, false, out);
                    Some(())
                }
                // `GeoLine.toValueString` of an undefined line.
                Value::Undefined if self.c.kind(id) == Kind::Line => {
                    out.push('?');
                    Some(())
                }
                Value::Circle { .. } | Value::Conic { .. } => self.conic(id, out),
                Value::Line { a, b, c }
                    if self.c.kind(id) == Kind::Line
                        && matches!(
                            self.c.equation_form(id),
                            None | Some(crate::kernel::EquationForm::Implicit)
                        ) =>
                {
                    implicit_line(a, b, c, self.numeric, out)
                }
                _ => None,
            },
        }
    }

    /// AlgoSolve prints a conic without a defining equation using the
    /// current GeoConic value, before parsing it back for the Giac template.
    fn conic(&self, id: crate::Id, out: &mut String) -> Option<()> {
        use crate::kernel::EquationForm;
        let conic = self.c.conic(id)?;
        let default = if self.c.kind(id) == Kind::Circle {
            EquationForm::Specific
        } else {
            EquationForm::Implicit
        };
        let style = self.c.equation_form(id).unwrap_or(&default);
        let fmt = |v: f64| format!("{}", v + 0.0);
        let value = crate::equation::styled_conic_equation(&conic, style, false, &fmt)
            .unwrap_or_else(|| crate::equation::conic_equation(&conic, &fmt));
        let (left, right) = value.split_once('=')?;
        self.computed_expression(left, out)?;
        out.push('=');
        self.computed_expression(right, out)
    }

    fn computed_expression(&self, text: &str, out: &mut String) -> Option<()> {
        let mut parsed = crate::expr::parse::parse_equation(self.c, text.trim()).ok()?;
        // The value text contains computed coefficients, which the reference
        // prints with 15 significant digits regardless of the display precision.
        parsed.lits.fill(Lit::Plain);
        let e = crate::expr::function_entry(self.c, parsed);
        self.expr(&e, e.root, out)
    }

    fn number(&self, v: f64, out: &mut String) {
        if self.numeric {
            numeric_number(v, out);
        } else {
            number(v, out);
        }
    }

    fn object_number(&self, id: crate::Id, value: f64, negated: bool, out: &mut String) {
        // The rewrite keeps the decimal of an unchanged typed GeoNumeric.
        // The solving templates print the computed numeric value.
        if let Some(decimal) = self
            .parameter
            .and_then(|_| crate::expr::free_decimal(self.c, id))
        {
            if negated {
                self.literal(&decimal.negated(), out);
            } else {
                self.literal(decimal, out);
            }
        } else {
            self.number(if negated { -value } else { value }, out);
        }
    }

    fn literal(&self, dec: &crate::expr::decimal::Dec, out: &mut String) {
        if self.numeric {
            out.push_str(&canonical_number(&decimal_text(dec)));
        } else {
            literal(dec, out);
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
                    Lit::Special(Some(dec), _) => self.literal(&dec.negated(), out),
                    _ => self.number(-v, out),
                }
                Some(())
            }
            Node::Obj(id) if self.row => {
                out.push_str("-(");
                self.object(id, out)?;
                out.push(')');
                Some(())
            }
            Node::Obj(id) => match self.c.value(id) {
                Value::Number(v) => {
                    self.object_number(id, v, true, out);
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
        self.bounded(|| self.expression(e, i, out))
    }

    fn expression(&self, e: &Entry, i: u32, out: &mut String) -> Option<()> {
        if let Some(result) = e.graph_rewrite(i) {
            let tree = result.as_ref()?.tree()?;
            return self.expr(tree, tree.root, out);
        }
        match e.nodes[i as usize] {
            Node::Num(v) => {
                match &e.lits[i as usize] {
                    Lit::Special(Some(dec), _) => self.literal(dec, out),
                    _ => self.number(v, out),
                }
                Some(())
            }
            Node::MinusOne => {
                out.push_str("-1");
                Some(())
            }
            Node::Var => {
                let parameter = self.parameter.unwrap_or("x");
                if !matches!(parameter, "x" | "y" | "z") {
                    out.push_str("ggbtmpvar");
                }
                out.push_str(parameter);
                Some(())
            }
            Node::Local(0, _) if self.parameter.is_some() => {
                out.push('x');
                Some(())
            }
            Node::Local(1, _) if self.parameter.is_some() => {
                out.push('y');
                Some(())
            }
            Node::SystemFunction(2, body) if self.parameter.is_some() => self.expr(e, body, out),
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
            Node::Obj(id) if self.row => self.object(id, out),
            Node::Obj(id) => match self.c.value(id) {
                Value::Number(v) => {
                    self.object_number(id, v, false, out);
                    Some(())
                }
                _ => None,
            },
            // `f(x)` and a function named in an expression print as the
            // function's expression.
            Node::FunctionObject(id) => self.object(id, out),
            Node::Apply(id, a) if e.nodes[a as usize] == Node::Var => {
                if self.parameter.is_some() {
                    return self.object(id, out);
                }
                out.push('(');
                self.object(id, out)?;
                out.push(')');
                Some(())
            }
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
                    Bin::Pow | Bin::RootPow => "^",
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

/// A line without a definition, as `GeoLine.toValueString` prints it in
/// implicit form (`Kernel.buildImplicitEquation` with cancelling) and the
/// reference's CAS reads it back: `a x + b y = -c`, integer coefficients
/// divided by their greatest common divisor, a coefficient 1 left out.
fn implicit_line(a: f64, b: f64, c: f64, numeric: bool, out: &mut String) -> Option<()> {
    let number = |v: f64, out: &mut String| {
        if numeric {
            numeric_number(v, out);
        } else {
            number(v, out);
        }
    };
    let is_zero = |v: f64| v.abs() < 1e-8;
    if is_zero(a) && is_zero(b) {
        out.push('?');
        return Some(());
    }
    // `Kernel.buildExplicitEquation` for a line parallel to an axis.
    if is_zero(b) {
        out.push_str("x=");
        number(-c / a + 0.0, out);
        return Some(());
    }
    if is_zero(a) {
        out.push_str("y=");
        number(-c / b + 0.0, out);
        return Some(());
    }
    let mut numbers = [a, b, c];
    if numbers.iter().all(|v| v.is_finite()) && numbers.iter().all(|v| (v - v.round()).abs() < 1e-8)
    {
        let g = numbers
            .iter()
            .map(|v| v.round().abs() as u128)
            .fold(0, crate::cas::integer_gcd);
        if g > 1 {
            for v in &mut numbers {
                *v /= g as f64;
            }
        }
    }
    let [a, b, c] = numbers;
    let zero = |v: f64| v.abs() <= 1e-15;
    let terms = [(a, "x"), (b, "y")];
    let leading = terms.iter().position(|(v, _)| !zero(*v))?;
    let mut lhs = String::new();
    let (v, name) = terms[leading];
    if v == 1.0 {
        lhs.push_str(name);
    } else if v == -1.0 {
        lhs.push_str(&format!("-({name})"));
    } else {
        lhs.push('(');
        number(v, &mut lhs);
        lhs.push_str(&format!(")*({name})"));
    }
    for &(v, name) in &terms[leading + 1..] {
        if zero(v) {
            continue;
        }
        let sign = if v < 0.0 { '-' } else { '+' };
        let magnitude = v.abs();
        let term = if magnitude == 1.0 {
            name.to_string()
        } else {
            let mut t = String::from("(");
            number(magnitude, &mut t);
            t.push_str(&format!(")*({name})"));
            t
        };
        lhs = format!("({lhs}){sign}({term})");
    }
    out.push_str(&lhs);
    out.push('=');
    number(-c + 0.0, out);
    Some(())
}

/// The text of a typed decimal.
fn decimal_text(dec: &crate::expr::decimal::Dec) -> String {
    match dec.signed_coefficient() {
        Some((coefficient, exp)) if (0..20).contains(&exp) => {
            (coefficient * 10i128.pow(exp as u32)).to_string()
        }
        Some((coefficient, exp)) if (-29..0).contains(&exp) => {
            let scale = 10i128.pow((-exp) as u32);
            let sign = if coefficient < 0 { "-" } else { "" };
            format!(
                "{sign}{}.{:0width$}",
                (coefficient / scale).abs(),
                (coefficient % scale).abs(),
                width = (-exp) as usize
            )
        }
        _ => format!("{}", dec.to_f64()),
    }
}

/// A computed number with 15 significant digits, rounded half up on its
/// exact value, with trailing zeros (`StringTemplate.prefixedDefaultSF`).
fn significant_text(v: f64) -> Option<String> {
    let digits = 14 - v.abs().log10().floor() as i64;
    if !(0..=40).contains(&digits) {
        return None;
    }
    let rounded = crate::expr::decimal::Dec::from_f64(v)?.set_scale_half_up(digits)?;
    let mut text = decimal_text(&rounded);
    let decimals = text.find('.').map_or(0, |dot| text.len() - dot - 1) as i64;
    if decimals == 0 && digits > 0 {
        text.push('.');
    }
    for _ in decimals..digits {
        text.push('0');
    }
    Some(text)
}

/// `StringUtil.canonicalNumber`: decimals that are all zeros are dropped.
fn canonical_number(text: &str) -> String {
    match text.find('.') {
        Some(dot) if text[dot + 1..].bytes().all(|b| b == b'0') => {
            if dot == 0 {
                "0".into()
            } else {
                text[..dot].to_string()
            }
        }
        Some(0) => format!("0{text}"),
        _ => text.to_string(),
    }
}

/// A computed number in the numeric template: its 15-significant-digit text.
fn numeric_number(v: f64, out: &mut String) {
    if !v.is_finite() {
        out.push('?');
        return;
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        out.push_str(&format!("{}", v as i64));
        return;
    }
    match significant_text(v) {
        Some(text) => out.push_str(&canonical_number(&text)),
        None => out.push_str(&format!("{v}")),
    }
}

/// A typed decimal (`MySpecialDouble`): an integer as written, otherwise
/// `(digits/10^k)`.
fn literal(dec: &crate::expr::decimal::Dec, out: &mut String) {
    match dec.signed_coefficient() {
        Some((coefficient, exp)) if (0..20).contains(&exp) => {
            if let Some(value) = coefficient.checked_mul(10i128.pow(exp as u32)) {
                out.push_str(&value.to_string());
            } else {
                out.push_str(&decimal_text(dec));
            }
        }
        // The sign stays with the text, also between -1 and 0 (`-0.5`).
        Some((_, exp)) if (-29..0).contains(&exp) => {
            out.push_str(&decimal_fraction(&decimal_text(dec)))
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
    match significant_text(v) {
        Some(text) => out.push_str(&decimal_fraction(&text)),
        None => out.push_str(&format!("{v}")),
    }
}

/// `convertScientificNotationGiac(canonicalNumber(text))` for a plain
/// decimal: `2.22` is `(222/100)`, `3.000` is `3`.
pub(super) fn decimal_fraction(text: &str) -> String {
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
    // CASgiac turns n_<index> into arbint(index). The algebra input
    // evaluates that arbitrary integer to zero.
    let mut out = String::with_capacity(ret.len());
    for token in ret.split_inclusive(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
        let name = token.trim_end_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
        if name
            .strip_prefix("n_")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        {
            out.push('0');
            out.push_str(&token[name.len()..]);
        } else {
            out.push_str(token);
        }
    }
    out
}

/// A function element read from Giac's output: the printed expression in
/// one free variable, which is the function's input.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Function {
    expr: Expr,
    variable: &'static str,
    text: String,
}

impl Function {
    pub(crate) fn value(&self, t: f64) -> f64 {
        let mut args = [f64::NAN; 3];
        args[match self.variable {
            "x" => 0,
            "y" => 1,
            _ => 2,
        }] = t;
        value_at(&self.expr, args).unwrap_or(f64::NAN)
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

/// Work units (term operations of the exact polynomial arithmetic) one
/// conversion of a result into a polynomial may spend.
const POLYNOMIAL_BUDGET: u64 = 1 << 20;

/// The value of a printed expression at `(x, y)`; `None` for anything the
/// reference would not read as a number.
fn value(e: &Expr, x: f64, y: f64) -> Option<f64> {
    value_at(e, [x, y, f64::NAN])
}

fn value_at(e: &Expr, args: [f64; 3]) -> Option<f64> {
    Some(match e {
        Expr::Num(v, _) => *v,
        Expr::Name(n) => match n.as_str() {
            "x" => args[0],
            "y" => args[1],
            "z" => args[2],
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "infinity" | "inf" => f64::INFINITY,
            _ => return None,
        },
        Expr::Neg(a) => -value_at(a, args)?,
        Expr::Bin(op, a, b) => {
            let (a, b) = (value_at(a, args)?, value_at(b, args)?);
            match op {
                b'+' => a + b,
                b'-' => a - b,
                b'*' => a * b,
                b'/' => a / b,
                _ => power(a, b),
            }
        }
        Expr::Call(name, operands) if operands.len() == 1 => {
            let operand = &operands[0];
            if matches!(name.as_str(), "re" | "im") {
                let (re, im) = real_and_imaginary(operand, args)?;
                return Some(if name == "re" { re } else { im });
            }
            let a = value_at(operand, args)?;
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

// PlotSolve wraps roots in re/im before it builds a coordinate pair.
fn real_and_imaginary(e: &Expr, args: [f64; 3]) -> Option<(f64, f64)> {
    let real = value_at(e, args)?;
    if !real.is_nan() {
        return Some((real, 0.0));
    }
    match e {
        Expr::Call(name, operands) if name == "sqrt" && operands.len() == 1 => {
            let base = value_at(&operands[0], args)?;
            Some((0.0, (-base).sqrt()))
        }
        Expr::Bin(b'^', base, exponent) => {
            let base = value_at(base, args)?;
            let exponent = value_at(exponent, args)?;
            let distance = (-base).powf(exponent);
            let (sin, cos) = (std::f64::consts::PI * exponent).sin_cos();
            Some((distance * cos, distance * sin))
        }
        _ => Some((real, real)),
    }
}

/// Whether every name and function of the expression is one the
/// reference's parser reads as a number or a variable.
fn known(e: &Expr) -> bool {
    match e {
        Expr::Num(..) => true,
        Expr::Name(n) => matches!(
            n.as_str(),
            "x" | "y" | "z" | "pi" | "e" | "inf" | "infinity"
        ),
        Expr::Neg(a) => known(a),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => known(a) && known(b),
        Expr::Call(name, args) => {
            args.len() == 1
                && matches!(
                    name.as_str(),
                    "sqrt" | "sin" | "cos" | "tan" | "exp" | "ln" | "log" | "abs" | "re" | "im"
                )
                && known(&args[0])
        }
        Expr::List(items) | Expr::Tuple(items) => items.iter().all(known),
    }
}

/// Algebra text for the coordinate expressions and equations read back
/// from Giac. The reader bounds the depth and the session bounds the size.
struct Algebra<'a>(&'a Expr, u8);

impl std::fmt::Display for Algebra<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::expr::commands::text::{format_number, superscript};

        let Self(expr, outer) = *self;
        let precedence = match expr {
            Expr::Bin(b'+' | b'-', ..) => 1,
            Expr::Bin(b'*' | b'/', ..) => 2,
            Expr::Neg(_) => 3,
            Expr::Bin(b'^', ..) => 4,
            _ => 5,
        };
        if precedence < outer {
            f.write_str("(")?;
        }
        match expr {
            Expr::Num(n, _) => f.write_str(&format_number(*n))?,
            Expr::Name(name) => f.write_str(match name.as_str() {
                "pi" => "π",
                "e" => "ℯ",
                "inf" | "infinity" => "∞",
                name => name,
            })?,
            Expr::Neg(a) => write!(f, "-{}", Self(a, precedence))?,
            Expr::Bin(b'^', a, b) => {
                write!(f, "{}", Self(a, precedence + 1))?;
                let exponent = match b.as_ref() {
                    Expr::Num(n, _) => Some(*n),
                    Expr::Neg(inner) => match inner.as_ref() {
                        Expr::Num(n, _) => Some(-n),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(digits) = exponent.and_then(|n| superscript(&n.to_string())) {
                    f.write_str(&digits)?;
                } else {
                    write!(f, "^({})", Self(b, 0))?;
                }
            }
            Expr::Bin(op, a, b) => {
                let (op, right) = match (*op, b.as_ref()) {
                    (b'+', Expr::Neg(right)) => (b'-', right.as_ref()),
                    (b'-', Expr::Neg(right)) => (b'+', right.as_ref()),
                    _ => (*op, b.as_ref()),
                };
                let separator = match op {
                    b'+' => " + ",
                    b'-' => " - ",
                    b'/' => " / ",
                    _ => {
                        let base = match right {
                            Expr::Bin(b'^', base, _) => base.as_ref(),
                            _ => right,
                        };
                        match (a.as_ref(), base) {
                            (Expr::Num(..), Expr::Name(_)) => "",
                            (_, Expr::Name(_)) => " ",
                            _ => " * ",
                        }
                    }
                };
                let left = Self(
                    a,
                    if op == b'/' {
                        precedence + 1
                    } else {
                        precedence
                    },
                );
                write!(f, "{left}{separator}{}", Self(right, precedence + 1))?;
            }
            Expr::Call(name, args) => {
                let name = match name.as_str() {
                    "re" => "real",
                    "im" => "imaginary",
                    "log" => "ln",
                    name => name,
                };
                f.write_str(name)?;
                algebra_sequence(f, args, "(", ")")?;
            }
            Expr::Tuple(items) => algebra_sequence(f, items, "(", ")")?,
            Expr::List(items) => algebra_sequence(f, items, "{", "}")?,
            Expr::Eq(a, b) => write!(f, "{} = {}", Self(a, 0), Self(b, 0))?,
        }
        if precedence < outer {
            f.write_str(")")?;
        }
        Ok(())
    }
}

fn algebra_sequence(
    f: &mut std::fmt::Formatter<'_>,
    items: &[Expr],
    open: &str,
    close: &str,
) -> std::fmt::Result {
    f.write_str(open)?;
    for (i, item) in items.iter().enumerate() {
        if i != 0 {
            f.write_str(", ")?;
        }
        write!(f, "{}", Algebra(item, 0))?;
    }
    f.write_str(close)
}

/// `a^b`, with the real root of a negative base for an exponent with an
/// odd denominator, as the reference's power does (`(-8)^(1/3)` is -2).
fn power(a: f64, b: f64) -> f64 {
    if a < 0.0 && b.fract() != 0.0 {
        for d in (3..100).step_by(2) {
            let n = b * f64::from(d);
            if (n - n.round()).abs() < 1e-12 {
                let magnitude = crate::expr::math::my_pow(-a, b);
                return if (n.round() as i64) % 2 == 0 {
                    magnitude
                } else {
                    -magnitude
                };
            }
        }
    }
    crate::expr::math::my_pow(a, b)
}

fn has_name(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Name(n) => n == name,
        Expr::Num(..) => false,
        Expr::Neg(a) => has_name(a, name),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => has_name(a, name) || has_name(b, name),
        Expr::Call(_, args) | Expr::List(args) | Expr::Tuple(args) => {
            args.iter().any(|a| has_name(a, name))
        }
    }
}

/// One element of the list `AlgebraProcessor.evaluateToList` builds.
fn element<const M: bool>(ctx: &Ctx<'_, M>, e: &Expr) -> Option<Slot> {
    if let Expr::Eq(left, right) = e {
        // A scalar assigned to a name outside the two-dimensional equation
        // variables has an equation definition but no defined line.
        if matches!(&**left, Expr::Name(name) if name.starts_with("ggbtmpvar"))
            && finite_number(right).is_some()
        {
            return Some(Slot::undefined(Kind::Line));
        }
    }
    if !known(e) && !matches!(e, Expr::List(_)) {
        // `fsolve(...)` left unevaluated, an unknown variable: the reference
        // reads an undefined object, which makes the list undefined.
        return None;
    }
    match e {
        Expr::List(items) => {
            let slots = items
                .iter()
                .map(|item| element(ctx, item))
                .collect::<Option<Vec<_>>>()?;
            let (start, len) = ctx.cas_slots(slots.into_iter())?;
            Some(Slot::list(start, len))
        }
        Expr::Eq(left, right) => equation_slot(left, right),
        Expr::Tuple(items) if items.len() == 2 => tuple_slot(&items[0], &items[1]),
        _ => {
            if let Some(variable) = free_variable(e) {
                return Some(function_slot(e, variable));
            }
            let n = value(e, 0.0, 0.0)?;
            (!n.is_nan()).then(|| Slot::number(n))
        }
    }
}

fn free_variable(e: &Expr) -> Option<&'static str> {
    let mut variables = ["x", "y", "z"].into_iter().filter(|name| has_name(e, name));
    let variable = variables.next()?;
    variables.next().is_none().then_some(variable)
}

/// ParametricProcessor first attempts an affine parametric line. Nonlinear
/// tuples become Cartesian curves over the algebra input's default domain.
fn tuple_slot(x: &Expr, y: &Expr) -> Option<Slot> {
    let tuple = Expr::Tuple(vec![x.clone(), y.clone()]);
    let Some(parameter) = free_variable(&tuple) else {
        return Some(Slot::point(Value::Point {
            x: finite_number(x)?,
            y: finite_number(y)?,
        }));
    };
    if let (Some([dx, x0]), Some([dy, y0])) = (affine(x, parameter), affine(y, parameter)) {
        return Some(
            Slot::line(Value::Line {
                a: -dy,
                b: dx,
                c: dy * x0 - dx * y0,
            })
            .with_symbolic(format!("X = ({}, {})", Algebra(x, 0), Algebra(y, 0))),
        );
    }
    let coordinate = |expr: &Expr| Function {
        expr: expr.clone(),
        variable: parameter,
        text: Algebra(expr, 0).to_string(),
    };
    Some(Slot::giac_curve([coordinate(x), coordinate(y)]))
}

fn affine(e: &Expr, variable: &str) -> Option<[f64; 2]> {
    if !has_name(e, variable) {
        return Some([0.0, finite_number(e)?]);
    }
    Some(match e {
        Expr::Name(_) => [1.0, 0.0],
        Expr::Neg(a) => affine(a, variable)?.map(|n| -n),
        Expr::Bin(op @ (b'+' | b'-'), a, b) => {
            let [p, q] = affine(a, variable)?;
            let [r, s] = affine(b, variable)?;
            let sign = if *op == b'+' { 1.0 } else { -1.0 };
            [p + sign * r, q + sign * s]
        }
        Expr::Bin(b'*', a, b) if !has_name(a, variable) => {
            let k = finite_number(a)?;
            affine(b, variable)?.map(|n| k * n)
        }
        Expr::Bin(b'*', a, b) if !has_name(b, variable) => {
            let k = finite_number(b)?;
            affine(a, variable)?.map(|n| k * n)
        }
        _ => return None,
    })
}

fn equation_slot(left: &Expr, right: &Expr) -> Option<Slot> {
    let has_z = has_name(left, "z") || has_name(right, "z");
    if left == right {
        return Some(Slot::undefined(Kind::Line));
    }
    if !has_z {
        if let Some(line) = line_slot(left, right) {
            return Some(line);
        }
    }
    if let Some(coefficients) = equation_coefficients(left, right) {
        if !has_z
            && coefficients
                .iter()
                .enumerate()
                .any(|(x, row)| row.iter().enumerate().any(|(y, n)| x + y == 2 && *n != 0.0))
            && coefficients
                .iter()
                .enumerate()
                .all(|(x, row)| row.iter().enumerate().all(|(y, n)| x + y <= 2 || *n == 0.0))
        {
            let coefficient = |x: usize, y: usize| {
                coefficients
                    .get(x)
                    .and_then(|row| row.get(y))
                    .copied()
                    .unwrap_or(0.0)
            };
            let mut conic = crate::algo::conic_nd::ConicData::new();
            conic.set_matrix([
                coefficient(2, 0),
                coefficient(0, 2),
                coefficient(0, 0),
                coefficient(1, 1) / 2.0,
                coefficient(1, 0) / 2.0,
                coefficient(0, 1) / 2.0,
            ]);
            let value = crate::algo::general_conic::value(Kind::Conic, &conic);
            let equation = format!("{} = {}", Algebra(left, 0), Algebra(right, 0));
            return Some(Slot::giac_conic(conic, value, equation));
        }
        if has_z || !matches!(left, Expr::Name(n) if n == "y" && !has_name(right, "y")) {
            let equation = format!("{} = {}", Algebra(left, 0), Algebra(right, 0));
            return Some(Slot::giac_implicit(coefficients, equation));
        }
    }
    if !has_z && matches!(left, Expr::Name(n) if n == "y") && !has_name(right, "y") {
        return Some(function_slot(right, "x"));
    }
    Some(Slot::undefined(Kind::ImplicitCurve))
}

fn equation_coefficients(left: &Expr, right: &Expr) -> Option<Vec<Vec<f64>>> {
    use crate::cas::Rat;

    let used = std::cell::Cell::new(0u64);
    let charge = |units| {
        used.set(used.get().saturating_add(units as u64));
        used.get() <= POLYNOMIAL_BUDGET
    };
    let work = &mut super::Work::new(&charge);
    let variables = &["x", "y", "z"];
    let q = quotient(left, variables, work)?.sub(quotient(right, variables, work)?, work)?;
    let scale = Rat::ONE.div(q.denominator.scalar()?)?;
    let mut poly = q.numerator.scale(scale, work)?.0;
    let (nx, ny) = poly.keys().try_fold((0, 0), |(nx, ny), &[x, y, z]| {
        (u32::from(x) + u32::from(y) + u32::from(z) <= crate::expr::equation::implicit::MAX_DEGREE)
            .then_some((nx.max(usize::from(x)), ny.max(usize::from(y))))
    })?;
    let mut matrix = vec![vec![0.0; ny + 1]; nx + 1];
    if poly.keys().any(|powers| powers[2] != 0) {
        // Polynomial.getCoeff("x", "y") assigns each term's coefficient
        // to its x/y powers, in order. A later monomial that differs only
        // in z overwrites the earlier entry instead of adding to it.
        let mut pending = vec![right, left];
        while let Some(term) = pending.pop() {
            work.spend(1)?;
            match term {
                Expr::Bin(b'+' | b'-', a, b) => pending.extend([&**b, &**a]),
                Expr::Neg(a) => pending.push(a),
                _ => {
                    for powers @ [x, y, _] in
                        quotient(term, variables, work)?.numerator.0.into_keys()
                    {
                        if let Some(coefficient) = poly.remove(&powers) {
                            matrix[usize::from(x)][usize::from(y)] = coefficient.to_f64();
                        }
                    }
                }
            }
        }
    } else {
        for ([x, y, _], coefficient) in poly {
            matrix[usize::from(x)][usize::from(y)] = coefficient.to_f64();
        }
    }
    Some(matrix)
}

/// The value of a constant result; an infinite or undefined value makes the
/// element undefined, as no recorded case has the reference read one.
fn finite_number(e: &Expr) -> Option<f64> {
    value(e, 0.0, 0.0).filter(|v| v.is_finite())
}

/// The coefficients of x, y and 1 of an expression linear in x and y.
fn linear(e: &Expr) -> Option<[f64; 3]> {
    let constant = |e: &Expr| {
        (!has_name(e, "x") && !has_name(e, "y"))
            .then(|| value(e, 0.0, 0.0))
            .flatten()
    };
    Some(match e {
        Expr::Name(n) if n == "x" => [1.0, 0.0, 0.0],
        Expr::Name(n) if n == "y" => [0.0, 1.0, 0.0],
        Expr::Neg(a) => linear(a)?.map(|v| -v),
        Expr::Bin(b'+', a, b) => {
            let (a, b) = (linear(a)?, linear(b)?);
            [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
        }
        Expr::Bin(b'-', a, b) => {
            let (a, b) = (linear(a)?, linear(b)?);
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
        }
        Expr::Bin(b'*', a, b) => match (constant(a), constant(b)) {
            (Some(k), _) => linear(b)?.map(|v| k * v),
            (_, Some(k)) => linear(a)?.map(|v| v * k),
            _ => return None,
        },
        Expr::Bin(b'/', a, b) => {
            let k = constant(b)?;
            linear(a)?.map(|v| v / k)
        }
        _ => [0.0, 0.0, constant(e)?],
    })
}

/// `left = right` as a line, when it is linear in x and y.
fn line_slot(left: &Expr, right: &Expr) -> Option<Slot> {
    let (l, r) = (linear(left)?, linear(right)?);
    let [a, b, c] = [l[0] - r[0], l[1] - r[1], l[2] - r[2]];
    if !(a.is_finite() && b.is_finite() && c.is_finite()) {
        return None;
    }
    if a == 0.0 && b == 0.0 {
        // `x = x`: a line without coefficients, which is undefined.
        return Some(Slot::undefined(Kind::Line));
    }
    Some(Slot::line(Value::Line { a, b, c }))
}

/// A function of `variable`: a polynomial when the expression is one, as
/// the kernel's own solver gives it, any other expression as printed.
fn function_slot(e: &Expr, variable: &'static str) -> Slot {
    let used = std::cell::Cell::new(0u64);
    let charge = |units| {
        used.set(used.get().saturating_add(units as u64));
        used.get() <= POLYNOMIAL_BUDGET
    };
    let work = &mut super::Work::new(&charge);
    if let Some(q) = quotient(e, &[variable], work) {
        if let Some(polynomial) = q
            .denominator
            .scalar()
            .and_then(|d| crate::cas::Rat::ONE.div(d))
            .and_then(|scale| q.numerator.scale(scale, work))
        {
            return Slot::polynomial(polynomial);
        }
    }
    Slot::giac_function(Function {
        expr: e.clone(),
        variable,
        text: String::new(),
    })
}

/// An exact rational function of `variable` (the function's input).
fn quotient(
    e: &Expr,
    variables: &[&str],
    work: &mut super::Work<'_>,
) -> Option<crate::cas::Quotient> {
    use crate::cas::{Poly, Quotient, Rat};
    Some(match e {
        Expr::Num(v, _) => Quotient::polynomial(Poly::constant(Rat::from_value(*v)?)),
        Expr::Name(n) => {
            if let Some(i) = variables.iter().position(|name| n == name) {
                Quotient::polynomial(Poly::variable(i))
            } else if matches!(n.as_str(), "pi" | "e") {
                Quotient::polynomial(Poly::constant(Rat::from_value(finite_number(e)?)?))
            } else {
                return None;
            }
        }
        Expr::Neg(a) => {
            let zero = Quotient::polynomial(Poly::constant(Rat::new(0, 1)?));
            zero.sub(quotient(a, variables, work)?, work)?
        }
        Expr::Bin(op, a, b) => {
            let (a, b) = (quotient(a, variables, work)?, quotient(b, variables, work)?);
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
        Expr::Call(..)
            if ["x", "y", "z"]
                .into_iter()
                .all(|variable| !has_name(e, variable)) =>
        {
            Quotient::polynomial(Poly::constant(Rat::from_value(finite_number(e)?)?))
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

    fn with_result(result: &str, check: impl FnOnce(crate::expr::List<'_>)) {
        let c = crate::script::load("f(x) = x").unwrap();
        let Def::Expr(def) = c.def(c.id("f").unwrap()) else {
            panic!("missing function definition");
        };
        let lists = crate::expr::eval::Lists::new(Vec::new());
        let ctx = Ctx::<true> {
            lists: Some(&lists),
            ..Ctx::new(
                &c,
                crate::expr::entry(&c, def),
                &crate::expr::eval::Env::None,
            )
        };
        let (start, len) = to_slots(&ctx, &post_process(result), false).unwrap();
        let slots = lists.into_arena();
        check(crate::expr::List::from_run(&slots, start, len));
    }

    #[test]
    fn plotsolve_reads_recorded_parametric_results() {
        use crate::expr::Element;
        // cas-plot-007: an affine parameterization gives a line.
        with_result("{(-3/2*y+3,0)}", |list| {
            assert!(matches!(
                list.get(0),
                Some(Element::Value(
                    Kind::Line,
                    Value::Line {
                        a: 0.0,
                        b: -1.5,
                        c: 0.0
                    }
                ))
            ));
        });
        // cas-plot-024: a variable in a denominator gives a curve.
        with_result("{(1/y,0)}", |list| {
            assert!(matches!(
                list.get(0),
                Some(Element::Value(
                    Kind::Curve,
                    Value::Curve {
                        start: -10.0,
                        end: 10.0
                    }
                ))
            ));
        });
        // cas-plot-023: a coordinate can contain the real or imaginary part of a root.
        with_result("{(re(√(-y^2+4)),im(√(-y^2+4)))}", |list| {
            let c = Construction::new();
            assert_eq!(list.curve_sample_at(&c, 0, 0.0), Some((2.0, 0.0)));
            assert_eq!(
                list.curve_sample_at(&c, 0, 3.0),
                Some((0.0, 5.0_f64.sqrt()))
            );
        });
        // cas-plot-025: the arbitrary integer is zero in the algebra view.
        with_result("{(n_0*pi,0)}", |list| {
            assert!(matches!(
                list.get(0),
                Some(Element::Value(Kind::Point, Value::Point { x: 0.0, y: 0.0 }))
            ));
        });
    }

    #[test]
    fn cas_curve_displays_log_as_natural_log() {
        with_result("{(log(y),0)}", |list| {
            assert_eq!(super::super::display_list(list), "{(ln(y), 0)}");
        });
    }

    #[test]
    fn cas_lists_display_cartesian_coordinates() {
        // cas-plot-024, as GeoCurveCartesian.toValueString displays it.
        with_result("{(1/y,0)}", |list| {
            assert_eq!(super::super::display_list(list), "{(1 / y, 0)}");
        });
        // cas-plot-023 also reads re/im of a square root as coordinates.
        with_result("{(re(√(-y^2+4)),im(√(-y^2+4)))}", |list| {
            assert_eq!(
                super::super::display_list(list),
                "{(real(sqrt(-y² + 4)), imaginary(sqrt(-y² + 4)))}"
            );
        });
    }

    #[test]
    fn cas_lists_display_the_conic_equation() {
        // cas-conic-060, as GeoConicND displays the user equation.
        with_result("{y=(x^2+x)}", |list| {
            assert_eq!(super::super::display_list(list), "{y = x² + x}");
        });
    }

    #[test]
    fn cas_lists_display_the_implicit_equation() {
        // cas-implicit-004, as GeoImplicitCurve displays the equation.
        with_result("{x=(y^3)}", |list| {
            assert_eq!(super::super::display_list(list), "{x = y³}");
        });
        // cas-freevariable-005 keeps a third variable in the display.
        with_result("{{x=(-z+1)}}", |list| {
            assert_eq!(super::super::display_list(list), "{{x = -z + 1}}");
        });
    }

    #[test]
    fn cas_lists_display_affine_tuples_as_parametric_lines() {
        // cas-plot-007, as ParametricProcessor sets the line's user form.
        with_result("{(-3/2*y+3,0)}", |list| {
            assert_eq!(super::super::display_list(list), "{X = (-3 / 2 y + 3, 0)}");
        });
    }

    #[test]
    fn solve_reads_recorded_quadratic_and_cubic_equations() {
        use crate::expr::Element;
        // cas-conic-060 and cas-implicit-004.
        for (result, kind) in [
            ("{y=(x^2+x)}", Kind::Conic),
            ("{x=(y^3)}", Kind::ImplicitCurve),
        ] {
            with_result(result, |list| {
                assert!(
                    matches!(list.get(0), Some(Element::Value(k, v)) if k == kind && v.is_defined())
                );
            });
        }
    }

    #[test]
    fn solves_with_a_third_free_variable_read_back_in_two_dimensions() {
        use crate::expr::Element;
        // cas-freevariable-005.
        with_result("{{-z+1,y,z}}", |list| {
            let Some(Element::List(row)) = list.get(0) else {
                panic!("expected a tuple of solutions");
            };
            for (i, value) in [-2.0, 3.0, 3.0].into_iter().enumerate() {
                assert_eq!(
                    row.function_sample_at(&Construction::new(), i, 3.0),
                    Some(value)
                );
            }
        });
        with_result("{{x=(-z+1),y=y,z=z}}", |list| {
            let Some(Element::List(row)) = list.get(0) else {
                panic!("expected a tuple of assignments");
            };
            assert!(matches!(
                row.get(0),
                Some(Element::Value(Kind::ImplicitCurve, Value::ImplicitCurve))
            ));
            assert!(matches!(
                row.get(2),
                Some(Element::Value(Kind::Line, Value::Undefined))
            ));
        });
    }

    #[test]
    fn implicit_coefficient_projection_keeps_the_last_term() {
        // cas-freevariable-010, the first two assignments printed by Giac.
        with_result("{x=(-2*z+1),y=z}", |list| {
            let c = Construction::new();
            assert_eq!(list.implicit_at(&c, 0).unwrap().0, &[vec![-1.0], vec![1.0]]);
            assert_eq!(list.implicit_at(&c, 1).unwrap().0, &[vec![-1.0, 1.0]]);
        });
    }

    #[test]
    fn translates_the_recorded_three_variable_system() {
        // cas-freevariable-005.
        let c = crate::script::load("r = Solutions({x+z=1}, {x,y,z})").unwrap();
        let Def::Expr(def) = c.def(c.id("r").unwrap()) else {
            panic!("expected a list expression");
        };
        let e = crate::expr::entry(&c, def);
        let Node::Cas(op, equations, _) = e.nodes[e.root as usize] else {
            panic!("expected a solving command");
        };
        let ctx = Ctx::<true>::new(&c, e, &crate::expr::eval::Env::None);
        // Tests also run without a native library, where the parser does not
        // retain Giac's hint text.
        let mut parsed = crate::expr::parse::parse(&c, "Solutions({x+z=1}, {x,y,z})").unwrap();
        let root = parsed.root as usize;
        parsed.lits[root] = Lit::Text("{x,y,z}".into());
        let e = crate::expr::function_entry(&c, parsed);
        let ctx = Ctx::<true> { e: &e, ..ctx };
        let (input, system) = translate(&ctx, equations, op).unwrap();
        assert!(system);
        assert_eq!(input, "ggbsort(normal(solve({(x)+(z)=1},{x, y, z},'!=')))");
    }

    #[test]
    fn constructed_conics_print_as_in_recorded_solving_inputs() {
        // cas-conic-057 and cas-order-097.
        let c = crate::script::load(
            "A = (1, 2)\nk = Circle(A, 3)\nF = (1, 1)\ng: x+y=-1\np = Parabola(F, g)",
        )
        .unwrap();
        let printer = Printer::new(&c, vec!["x".into(), "y".into(), "z".into()], false);
        let mut text = String::new();
        printer.object(c.id("k").unwrap(), &mut text).unwrap();
        assert_eq!(text, "(((x)-(1))^(2))+(((y)-(2))^(2))=9");
        text.clear();
        printer.object(c.id("p").unwrap(), &mut text).unwrap();
        assert_eq!(
            text,
            "(((((x)^(2))-(((2)*(x))*(y)))+((y)^(2)))-((6)*(x)))-((6)*(y))=-3"
        );
    }

    #[test]
    fn constructed_conics_keep_the_recorded_computed_coefficients() {
        // cas-order-094, cas-order-096 and cas-order-097.
        for (script, id, expected) in [
            ("A = (0.37, -1.21)\nk = Circle(A, 1.7)", "k", "(((x)-((370000000000000/1000000000000000)))^(2))+(((y)+((121000000000000/100000000000000)))^(2))=(289000000000000/100000000000000)"),
            ("A = (1, 2)\nB = (3, -1)\nC = (-2, 0.5)\nD = (0.5, 3)\nE = (-1, -2)\nk = Conic(A, B, C, D, E)", "k", "((((((-513750000000000/1000000000000))*((x)^(2)))-((((594375000000000/1000000000000))*(x))*(y)))-(((862500000000000/10000000000000))*((y)^(2))))+(((251250000000000/1000000000000))*(x)))-(((125625000000000/1000000000000))*(y))=(-204750000000000/100000000000)"),
            ("F = (1, 1)\ng: x+y=-1\nk = Parabola(F, g)\nP = (0, 0)\nQ = (2, 1)\nR = (3, 2)\nh = Ellipse(P, Q, R)", "h", "((((((847921561087423/10000000000000))*((x)^(2)))-(((16)*(x))*(y)))+(((967921561087423/10000000000000))*((y)^(2))))-(((161584312217485/1000000000000))*(x)))-(((807921561087423/10000000000000))*(y))=(407960780543712/1000000000000)"),
        ] {
            let c = crate::script::load(script).unwrap();
            let printer = Printer::new(&c, Vec::new(), false);
            let mut text = String::new();
            printer.object(c.id(id).unwrap(), &mut text).unwrap();
            assert_eq!(text, expected, "{script}");
        }
    }

    #[test]
    fn radicals_in_a_free_variable_stay_variable() {
        // cas-shape-024.
        with_result("{{√(-x^2+4)},{-√(-x^2+4)}}", |list| {
            let c = Construction::new();
            let Some(crate::expr::Element::List(row)) = list.get(0) else {
                panic!("expected one row per solution");
            };
            assert!(row.function_sample_at(&c, 0, -8.0).unwrap().is_nan());
            assert_eq!(row.function_sample_at(&c, 0, 0.0), Some(2.0));
        });
    }

    #[test]
    fn repeated_subtrees_cannot_expand_into_an_oversized_request() {
        let c = Construction::new();
        let mut builder = crate::expr::equation::Builder::empty();
        let mut root = builder.push(Node::Var);
        for _ in 0..18 {
            root = builder.push(Node::Bin(Bin::Add, root, root));
        }
        let e = crate::expr::function_entry(&c, builder.finish(&c, root).unwrap());
        let printer = Printer::new(&c, Vec::new(), false);
        let mut text = String::new();
        assert!(printer.expr(&e, root, &mut text).is_none());
        assert!(text.len() < 64 * 1024);
    }

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
    fn typed_decimals_keep_their_sign() {
        let print = |text: &str| {
            let mut s = String::new();
            literal(&crate::expr::decimal::Dec::parse(text).unwrap(), &mut s);
            s
        };
        assert_eq!(print("-0.5"), "(-05/10)");
        assert_eq!(print("-0.25"), "(-025/100)");
        assert_eq!(print("-1.5"), "(-15/10)");
        assert_eq!(print("0.5"), "(5/10)");
        assert_eq!(print("84.36"), "(8436/100)");
        assert_eq!(print("-3"), "-3");
    }

    #[test]
    fn templates_take_their_arguments() {
        assert_eq!(
            fill(SOLVE_1, &["x=3"], &["x=3"]),
            "ggbsort(normal([op(solve(x=3))]))"
        );
        assert_eq!(fill("f(%0,%%0)", &["(1/2)"], &["0.5"]), "f((1/2),0.5)");
    }

    #[test]
    fn malformed_output_reads_as_nothing() {
        for text in [
            "", "{", "{1,", "(1,2", "{x=}", "1 2", "{1}}", "√", "x=y=1", "@",
        ] {
            assert!(parse(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn deep_nesting_reads_as_nothing_without_overflowing() {
        let deep = format!("{}x{}", "(".repeat(10_000), ")".repeat(10_000));
        assert!(parse(&deep).is_none());
        let signs = format!("{}1", "-".repeat(10_000));
        assert!(parse(&signs).is_none());
        let powers = vec!["2"; 10_000].join("^");
        assert!(parse(&powers).is_none());
        // A chain of sums builds a deep tree without recursing in the reader.
        let chain = vec!["1"; 10_000].join("+");
        assert!(parse(&chain).is_none());
        let fine = format!("{}x{}", "(".repeat(20), ")".repeat(20));
        assert!(parse(&fine).is_some());
    }

    #[test]
    fn a_huge_power_stops_at_the_work_budget() {
        let e = parse("(((x+1)^64)^64)^64").unwrap();
        let used = std::cell::Cell::new(0u64);
        let charge = |units| {
            used.set(used.get().saturating_add(units as u64));
            used.get() <= POLYNOMIAL_BUDGET
        };
        let work = &mut super::super::Work::new(&charge);
        let start = std::time::Instant::now();
        assert!(quotient(&e, &["x"], work).is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        // The function element still samples Giac's printed form.
        assert!(function_slot(&e, "x").numeric_cell().is_none());
    }

    #[test]
    fn non_finite_results_are_undefined() {
        for text in ["1/0", "-1/0", "0/0", "10^400", "sqrt(-1)", "ln(0)"] {
            assert_eq!(finite_number(&parse(text).unwrap()), None, "{text}");
        }
        assert_eq!(finite_number(&parse("-√4+1").unwrap()), Some(-1.0));
    }

    #[test]
    fn unknown_names_and_calls_are_not_known() {
        for text in ["fsolve(x^2=2,x)", "ggbtmpvart", "{n_0*pi}", "f(1,2)"] {
            assert!(!known(&parse(text).unwrap()), "{text}");
        }
        assert!(known(&parse("{x=-√3,(1,2),sin(pi)}").unwrap()));
    }
}
