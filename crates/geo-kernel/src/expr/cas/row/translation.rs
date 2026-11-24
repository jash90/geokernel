//! Ggb2giac templates used by GeoCasCell (GeoGebra 112919f, EUPL 1.2).

#[cfg(test)]
mod command_tests;
mod commands;

use super::{
    cell_equation, display, is_feature_command, parse, CasRow, Expr, BUILTINS, MAX_INPUT_BYTES,
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
    normalize(&mut value, &mut constants, numeric == Some(1));
    let value = cell_equation(value);
    Some(if numeric.is_some() {
        display::render_numeric(&value)
    } else {
        display::render(&value)
    })
}

/// CASgiac removes dummy-variable prefixes and GeoCasCell gives each
/// arbitrary integration constant an index within the cell.
fn normalize(tree: &mut Expr, constants: &mut Vec<String>, round: bool) {
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
        Expr::Num(n, text) => {
            if round && n.is_finite() {
                *text = Some(display::rounded(*n));
            }
        }
        Expr::Neg(child) => normalize(child, constants, round),
        Expr::Eq(a, b) => {
            normalize(a, constants, round);
            normalize(b, constants, round);
        }
        Expr::Bin(op, a, b) => {
            normalize(a, constants, round);
            normalize(b, constants, round);
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
                normalize(item, constants, round);
            }
        }
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                normalize(item, constants, round);
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
        Expr::Call(name, _) if name != "Substitute" && !BUILTINS.contains(&name.as_str()) => source,
        _ => EVALUATE.replace("%0", &source),
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

struct Translator<'a> {
    c: &'a Construction,
    command_index: usize,
}

impl Translator<'_> {
    fn print(&mut self, tree: &Expr) -> Option<String> {
        let text = match tree {
            Expr::Num(_, Some(text)) => {
                if text.bytes().all(|byte| byte.is_ascii_digit()) {
                    text.clone()
                } else {
                    giac::decimal_fraction(text)
                }
            }
            Expr::Num(..) => return None,
            Expr::Name(name) => match self.c.resolve_label(name) {
                Some(id) => giac::row_object(self.c, id)?,
                None => variable(name),
            },
            Expr::Neg(a) => format!("-({})", self.print(a)?),
            Expr::Bin(op, a, b) => {
                if let Some(op) = super::super::syntax::relation(*op) {
                    format!("{} {op} {}", self.print(a)?, self.print(b)?)
                } else {
                    format!("({}){}({})", self.print(a)?, *op as char, self.print(b)?)
                }
            }
            Expr::Eq(a, b) => {
                let left = match &**a {
                    Expr::Call(name, args) => {
                        format!("{}({})", variable(name), self.arguments(args, ",")?)
                    }
                    _ => self.print(a)?,
                };
                format!("{left}={}", self.print(b)?)
            }
            Expr::List(items) => format!("{{{}}}", self.arguments(items, ", ")?),
            Expr::Tuple(items) => format!("({})", self.arguments(items, ", ")?),
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
                return Some(format!("{name}({})", self.print(arg)?));
            }
        }

        self.command_index += 1;
        let index = self.command_index;
        match (name, args) {
            ("Evaluate", [arg]) => Some(EVALUATE.replace("%0", &self.print(arg)?)),
            ("NextPrime", [arg]) => Some(format!("nextprime({})", self.print(arg)?)),
            ("PreviousPrime", [arg]) => Some(format!(
                "[[ggbpparg0:={}],if (ggbpparg0>2) then prevprime(ggbpparg0) else 0/0 fi][1]",
                self.print(arg)?
            )),
            ("Solve" | "Solutions" | "NSolve" | "NSolutions", [arg]) => {
                Some(giac::row_solve(name, &self.print(arg)?, "x"))
            }
            ("Solve" | "Solutions" | "NSolve" | "NSolutions", [arg, variable]) => Some(
                giac::row_solve(name, &self.print(arg)?, &self.print(variable)?),
            ),
            (name, [arg]) if Op::named(name).is_some_and(Op::is_giac_rewrite) => {
                let text = giac::rewrite::input(Op::named(name)?, &self.print(arg)?)?;
                Some(text.replace("ggbsimparg01", &format!("ggbsimparg0{index}")))
            }
            _ => self.cas_only_command(name, args, index),
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
        if template.contains("%[1..]") {
            let argument = printed.first()?;
            let source = if printed.len() == 1 {
                argument.clone()
            } else {
                format!("[{}]", printed.join(","))
            };
            return Some(template.replace("%0", argument).replace('%', &source));
        }
        let arguments: Vec<_> = printed.iter().map(String::as_str).collect();
        Some(giac::row_fill(&template, &arguments))
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
            Some(giac::row_solve("Solve", "(x)^(2)=1", "x")),
        );
    }

    #[test]
    fn cas_cell_refuses_unknown_calls_before_dispatch() {
        let c = Construction::new();
        assert!(translate(&c, &parse("unknown(1)", true).unwrap()).is_none());
    }
}
