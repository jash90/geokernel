//! Ggb2giac templates used by GeoCasCell (GeoGebra 112919f, EUPL 1.2).

#[cfg(test)]
mod command_tests;
mod commands;

use super::{
    cell_equation, display, is_feature_command, parse, solve_variable, CasRow, Expr, BUILTINS,
    MAX_INPUT_BYTES,
};
use crate::expr::cas::{giac, Op};
use crate::Construction;

const EVALUATE: &str = r#"[[[ggbevalarg:=0/0],[ggbevalarg:=%0],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(""+ggbevalans)<=length(""+ggbevalans2),ggbevalans,ggbevalans2)][1]"#;

pub(super) fn evaluate(c: &Construction, tree: &Expr) -> Option<CasRow> {
    let input = match translate(c, tree) {
        Some(input) => input,
        None if matches!(tree, Expr::Call(name, _) if is_feature_command(name)) => {
            // GeoCasCell's failed evaluation and fallback.
            return Some(CasRow {
                text: "Sorry, something went wrong. Please check your input".into(),
                latex: String::new(),
                exact: None,
            });
        }
        None => return None,
    };
    let raw = giac::evaluate_raw(&input)?;
    read_back(tree, &raw)
}

fn read_back(input: &Expr, raw: &str) -> Option<CasRow> {
    let mut value = parse(raw, false)?;
    let mut constants = Vec::new();
    let numeric = match input {
        Expr::Call(name, args) if name == "Numeric" => Some(args.len()),
        _ => None,
    };
    normalize(&mut value, &mut constants);
    let value = cell_equation(value);
    Some(if let Some(arity) = numeric {
        display::render_numeric(&value, arity > 1)
    } else {
        display::render(&value)
    })
}

/// CASgiac removes dummy-variable prefixes and GeoCasCell gives each
/// arbitrary integration constant an index within the cell.
fn normalize(tree: &mut Expr, constants: &mut Vec<String>) {
    let name = |name: &mut String| {
        if let Some(original) = name.strip_prefix("ggbtmpvar") {
            *name = original.to_owned();
        }
    };
    match tree {
        Expr::Name(n) => {
            if n.strip_prefix("c_").is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            }) {
                let index = constants
                    .iter()
                    .position(|constant| constant == n)
                    .unwrap_or_else(|| {
                        constants.push(n.clone());
                        constants.len() - 1
                    });
                *n = format!("c_{}", index + 1);
            } else {
                name(n);
            }
        }
        Expr::Num(..) => {}
        Expr::Neg(child) => normalize(child, constants),
        Expr::Eq(a, b) => {
            normalize(a, constants);
            normalize(b, constants);
        }
        Expr::Bin(op, a, b) => {
            normalize(a, constants);
            normalize(b, constants);
            // GeoGebra reads i*pi/n as i times the real fraction pi/n.
            if *op == b'/' {
                if let Expr::Bin(b'*', i, real) = &mut **a {
                    if matches!(i.as_ref(), Expr::Name(n) if n == "ί") {
                        std::mem::swap(i, b);
                        std::mem::swap(i, real);
                        std::mem::swap(a, b);
                        let Expr::Bin(inner, _, _) = &mut **b else {
                            unreachable!()
                        };
                        std::mem::swap(op, inner);
                    }
                }
            }
        }
        Expr::Call(n, items) => {
            name(n);
            for item in items {
                normalize(item, constants);
            }
        }
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                normalize(item, constants);
            }
        }
    }
}

fn translate(c: &Construction, tree: &Expr) -> Option<String> {
    let mut printer = Translator {
        c,
        command_index: 0,
    };
    let source = printer.print(tree)?;
    let translated = match tree {
        Expr::Call(name, _)
            if !matches!(name.as_str(), "Substitute" | "!")
                && !BUILTINS.contains(&name.as_str())
                && function(c, name).is_none() =>
        {
            source
        }
        _ => giac::row_fill(EVALUATE, &[&source])?,
    };
    (translated.len() <= MAX_INPUT_BYTES).then_some(translated)
}

fn variable(name: &str) -> String {
    match name {
        "x" | "y" | "z" | "pi" | "e" | "?" => name.into(),
        "π" => "pi".into(),
        "ℯ" => "e".into(),
        "ί" => "i".into(),
        _ => format!("ggbtmpvar{name}"),
    }
}

fn literal(text: &str) -> Option<String> {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        return Some(giac::decimal_fraction(text));
    };
    let exponent: i64 = exponent.parse().ok()?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some("0".into());
    }
    let exponent = exponent.checked_sub(fraction.len() as i64)?;
    let zeros = usize::try_from(exponent.unsigned_abs()).ok()?;
    if digits.len().checked_add(zeros)?.checked_add(4)? > MAX_INPUT_BYTES {
        return None;
    }
    if exponent >= 0 {
        let mut value = String::with_capacity(digits.len() + zeros);
        value.push_str(digits);
        value.extend(std::iter::repeat_n('0', zeros));
        Some(value)
    } else {
        Some(format!("({digits}/1{})", "0".repeat(zeros)))
    }
}

fn function(c: &Construction, name: &str) -> Option<crate::Id> {
    let id = c.resolve_label(name)?;
    matches!(
        c.kind(id),
        crate::Kind::Function | crate::Kind::FunctionNVar
    )
    .then_some(id)
}

fn binary(op: u8, lhs: &str, rhs: &str) -> Option<String> {
    (lhs.len() + rhs.len() + 5 <= MAX_INPUT_BYTES).then(|| format!("({lhs}){}({rhs})", op as char))
}

fn factorial(value: &str) -> Option<String> {
    let template = if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        "%0!"
    } else {
        "(%0)!"
    };
    giac::row_fill(template, &[value])
}

fn argument_variable(c: &Construction, arg: &Expr, source: &str) -> String {
    // A named equation acquires its Cartesian variables when printed.
    if matches!(arg, Expr::Name(label) if c.resolve_label(label).is_some()) && source.contains('=')
    {
        if let Some(axis) = ["x", "y", "z"]
            .into_iter()
            .find(|axis| giac::has_variable(source, axis))
        {
            return axis.into();
        }
    }
    variable(solve_variable(c, arg))
}

struct Translator<'a> {
    c: &'a Construction,
    command_index: usize,
}

impl Translator<'_> {
    fn print(&mut self, tree: &Expr) -> Option<String> {
        let text = match tree {
            Expr::Num(_, Some(text)) => literal(text)?,
            Expr::Num(..) => return None,
            Expr::Name(name) => match self.c.resolve_label(name) {
                Some(id) => giac::row_object(self.c, id)?,
                None => variable(name),
            },
            Expr::Neg(a) => giac::row_fill("-(%0)", &[&self.print(a)?])?,
            Expr::Bin(op, a, b) => {
                if *op == b'&' {
                    giac::row_fill("(%0 && %1)", &[&self.print(a)?, &self.print(b)?])?
                } else if let Some(op) = super::super::syntax::relation(*op) {
                    giac::row_fill("%0 %1 %2", &[&self.print(a)?, op, &self.print(b)?])?
                } else {
                    binary(*op, &self.print(a)?, &self.print(b)?)?
                }
            }
            Expr::Eq(a, b) => {
                let left = match &**a {
                    Expr::Call(name, args) if name != "!" && !BUILTINS.contains(&name.as_str()) => {
                        giac::row_fill("%0(%1)", &[&variable(name), &self.arguments(args, ",")?])?
                    }
                    _ => self.print(a)?,
                };
                giac::row_fill("%0=%1", &[&left, &self.print(b)?])?
            }
            Expr::List(items) => giac::row_fill("{%0}", &[&self.arguments(items, ", ")?])?,
            Expr::Tuple(items) => giac::row_fill("(%0)", &[&self.arguments(items, ", ")?])?,
            Expr::Call(name, args) => self.command(name, args)?,
        };
        (text.len() <= MAX_INPUT_BYTES).then_some(text)
    }

    fn arguments(&mut self, args: &[Expr], separator: &str) -> Option<String> {
        let mut out = String::new();
        for (index, arg) in args.iter().enumerate() {
            let text = self.print(arg)?;
            if out.len() + text.len() + separator.len() > MAX_INPUT_BYTES {
                return None;
            }
            if index != 0 {
                out.push_str(separator);
            }
            out.push_str(&text);
        }
        Some(out)
    }

    fn command(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        if let [arg] = args {
            if BUILTINS.contains(&name) {
                let name = if name == "ln" { "log" } else { name };
                return giac::row_fill("%0(%1)", &[name, &self.print(arg)?]);
            }
        }

        self.command_index += 1;
        let index = self.command_index;
        match (name, args) {
            ("!", [arg]) => factorial(&self.print(arg)?),
            // GeoCasCell's algebra fallback drops an unsupported Numeric wrapper.
            ("Numeric", [first, _, _, ..]) => giac::row_fill(EVALUATE, &[&self.print(first)?]),
            ("Evaluate", [arg]) => giac::row_fill(EVALUATE, &[&self.print(arg)?]),
            ("NextPrime", [arg]) => giac::row_fill("nextprime(%0)", &[&self.print(arg)?]),
            ("PreviousPrime", [arg]) => giac::row_fill(
                "[[ggbpparg0:=%0],if (ggbpparg0>2) then prevprime(ggbpparg0) else 0/0 fi][1]",
                &[&self.print(arg)?],
            ),
            ("Solutions", [arg]) => giac::row_solutions(&self.print(arg)?),
            ("Solve" | "NSolve" | "NSolutions", [arg]) => {
                let source = self.print(arg)?;
                let variable = argument_variable(self.c, arg, &source);
                giac::row_solve(name, &source, &variable)
            }
            ("Solve" | "Solutions" | "NSolve" | "NSolutions", [arg, variable]) => {
                giac::row_solve(name, &self.print(arg)?, &self.print(variable)?)
            }
            (name, [arg]) if Op::named(name).is_some_and(Op::is_giac_rewrite) => {
                giac::rewrite::row_input(Op::named(name)?, &self.print(arg)?, index)
            }
            _ if is_feature_command(name) => self.cas_only_command(name, args, index),
            _ => {
                let id = function(self.c, name)?;
                let mut remaining = MAX_INPUT_BYTES;
                let arguments = args
                    .iter()
                    .map(|arg| {
                        let value = self.print(arg)?;
                        remaining = remaining.checked_sub(value.len())?;
                        Some(value)
                    })
                    .collect::<Option<Vec<_>>>()?;
                giac::row_application(self.c, id, arguments)
            }
        }
    }

    fn cas_only_command(&mut self, name: &str, args: &[Expr], index: usize) -> Option<String> {
        let template = commands::template(name, args.len())?;
        let template = template
            .replace("ggblaparg0", &format!("ggblaparg0{index}"))
            .replace("ggbratarg0", &format!("ggbratarg0{index}"));
        let mut remaining = MAX_INPUT_BYTES;
        let printed = args
            .iter()
            .map(|arg| {
                let arg = self.print(arg)?;
                remaining = remaining.checked_sub(arg.len())?;
                Some(arg)
            })
            .collect::<Option<Vec<_>>>()?;
        let arguments: Vec<_> = printed.iter().map(String::as_str).collect();
        if template.contains("%[1..]") {
            let first = *arguments.first()?;
            let template = template.replace("%[1..]", "%1[1..]");
            if arguments.len() == 1 {
                return giac::row_fill(&template, &[first, first]);
            }
            let separators = arguments.len() - 1;
            remaining.checked_sub(separators + 2)?;
            let all = format!("[{}]", arguments.join(","));
            return giac::row_fill(&template, &[first, &all]);
        }
        giac::row_fill(&template, &arguments)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_cell_translates_the_recorded_evaluate_and_solve_forms() {
        let c = Construction::new();
        assert_eq!(
            translate(&c, &parse("sqrt(2) + sqrt(3)", true).unwrap()).unwrap(),
            r#"[[[ggbevalarg:=0/0],[ggbevalarg:=(sqrt(2))+(sqrt(3))],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(""+ggbevalans)<=length(""+ggbevalans2),ggbevalans,ggbevalans2)][1]"#,
        );
        assert_eq!(
            translate(&c, &parse("Solve(x^2=1)", true).unwrap()),
            giac::row_solve("Solve", "(x)^(2)=1", "x"),
        );
    }

    #[test]
    fn translates_exact_literals_and_construction_definitions() {
        let c = crate::script::load("a = 1/3").unwrap();
        for (source, expected) in [
            ("3*a", "(3)*((1)/(3))"),
            ("1.2E3 + 0", "(1200)+(0)"),
            ("1.2E-3", "(12/10000)"),
            ("5!", "5!"),
            ("x² - 1", "((x)^(2))-(1)"),
        ] {
            let tree = parse(source, true).unwrap();
            let mut printer = Translator {
                c: &c,
                command_index: 0,
            };
            assert_eq!(printer.print(&tree).as_deref(), Some(expected), "{source}");
        }
    }

    #[test]
    fn translates_a_solve_with_the_equations_variable() {
        let c = Construction::new();
        let tree = parse("Solve(y = 1)", true).unwrap();
        assert_eq!(translate(&c, &tree), giac::row_solve("Solve", "y=1", "y"));
    }

    #[test]
    fn cas_cell_refuses_unknown_calls_before_dispatch() {
        let c = Construction::new();
        assert!(translate(&c, &parse("unknown(1)", true).unwrap()).is_none());
    }

    #[test]
    fn translated_named_factorial_keeps_the_sign_inside() {
        let c = crate::script::load("a = -1").unwrap();
        let tree = parse("a!", true).unwrap();
        let mut printer = Translator {
            c: &c,
            command_index: 0,
        };
        assert_eq!(printer.print(&tree).as_deref(), Some("(-1)!"));
    }

    #[test]
    fn translated_named_equation_selects_its_variable() {
        let c = crate::script::load("g: y = 1").unwrap();
        let tree = parse("Solve(g)", true).unwrap();
        let expected = giac::row_solve("Solve", "y=1", "y").unwrap();
        assert_eq!(translate(&c, &tree).as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn binary_translation_checks_its_size_before_assembly() {
        let operand = literal("1E40000").unwrap();
        assert!(binary(b'+', &operand, &operand).is_none());
        let fits = "1".repeat(MAX_INPUT_BYTES - 6);
        assert_eq!(binary(b'+', &fits, "0").unwrap().len(), MAX_INPUT_BYTES);
    }

    #[test]
    fn row_solve_checks_repetitions_before_assembly() {
        let rhs = literal("1E10000").unwrap();
        let equation = format!("x={rhs}");
        assert!(giac::row_solve("Solve", &equation, "x").is_none());
        assert!(giac::row_solve("Solve", "x=1", "x").is_some());
    }
}
