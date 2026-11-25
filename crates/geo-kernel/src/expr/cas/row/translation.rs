//! Ggb2giac templates used by GeoCasCell (GeoGebra 112919f, EUPL 1.2).

use super::{parse, solve_variable, Expr, BUILTINS, MAX_INPUT_BYTES};
use crate::expr::cas::{giac, Op};
use crate::Construction;

const EVALUATE: &str = r#"[[[ggbevalarg:=0/0],[ggbevalarg:=%0],[ggbevalans:=regroup(normal(ggbevalarg))],[ggbevalans2:=regroup(ggbevalarg)]],when(length(""+ggbevalans)<=length(""+ggbevalans2),ggbevalans,ggbevalans2)][1]"#;

pub(super) fn evaluate(c: &Construction, tree: &Expr) -> Option<Expr> {
    let input = translate(c, tree)?;
    let raw = giac::evaluate_raw(&input)?;
    // CASgiac removes the dummy-variable prefix before parsing the result.
    parse(&raw.replace("ggbtmpvar", ""), false)
}

fn translate(c: &Construction, tree: &Expr) -> Option<String> {
    let source = print(c, tree)?;
    let translated = match tree {
        Expr::Call(name, _) if name != "!" && !BUILTINS.contains(&name.as_str()) => source,
        _ => EVALUATE.replace("%0", &source),
    };
    (translated.len() <= MAX_INPUT_BYTES).then_some(translated)
}

fn variable(name: &str) -> String {
    if matches!(name, "x" | "y" | "z" | "pi" | "e" | "?") {
        name.into()
    } else {
        format!("ggbtmpvar{name}")
    }
}

fn print(c: &Construction, tree: &Expr) -> Option<String> {
    let text = match tree {
        Expr::Num(_, Some(text)) => literal(text)?,
        Expr::Num(..) => return None,
        Expr::Name(name) => match c.resolve_label(name) {
            Some(id) => giac::row_object(c, id)?,
            None => variable(name),
        },
        Expr::Neg(a) => format!("-({})", print(c, a)?),
        Expr::Bin(op, a, b) => format!("({}){}({})", print(c, a)?, *op as char, print(c, b)?),
        Expr::Eq(a, b) => {
            let left = match &**a {
                Expr::Call(name, args) => {
                    format!("{}({})", variable(name), arguments(c, args)?)
                }
                _ => print(c, a)?,
            };
            format!("{left}={}", print(c, b)?)
        }
        Expr::List(items) => format!("{{{}}}", arguments(c, items)?),
        Expr::Tuple(items) => format!("({})", arguments(c, items)?),
        Expr::Call(name, args) => command(c, name, args)?,
    };
    (text.len() <= MAX_INPUT_BYTES).then_some(text)
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

fn arguments(c: &Construction, args: &[Expr]) -> Option<String> {
    let mut out = String::new();
    for (index, arg) in args.iter().enumerate() {
        let text = print(c, arg)?;
        if out.len() + text.len() + 1 > MAX_INPUT_BYTES {
            return None;
        }
        if index != 0 {
            out.push(',');
        }
        out.push_str(&text);
    }
    Some(out)
}

fn command(c: &Construction, name: &str, args: &[Expr]) -> Option<String> {
    match (name, args) {
        ("!", [arg]) => {
            let value = print(c, arg)?;
            Some(
                if matches!(arg, Expr::Num(..) | Expr::Name(_)) && !value.starts_with('(') {
                    format!("{value}!")
                } else {
                    format!("({value})!")
                },
            )
        }
        ("Evaluate", [arg]) => Some(EVALUATE.replace("%0", &print(c, arg)?)),
        ("NextPrime", [arg]) => Some(format!("nextprime({})", print(c, arg)?)),
        ("PreviousPrime", [arg]) => Some(format!(
            "[[ggbpparg0:={}],if (ggbpparg0>2) then prevprime(ggbpparg0) else 0/0 fi][1]",
            print(c, arg)?
        )),
        ("Solve" | "Solutions" | "NSolve" | "NSolutions", [arg]) => {
            let variable = variable(solve_variable(c, arg));
            Some(giac::row_solve(name, &print(c, arg)?, &variable))
        }
        ("Solve" | "Solutions" | "NSolve" | "NSolutions", [arg, variable]) => {
            Some(giac::row_solve(name, &print(c, arg)?, &print(c, variable)?))
        }
        (name, [arg]) if Op::named(name).is_some_and(Op::is_giac_rewrite) => {
            giac::rewrite::input(Op::named(name)?, &print(c, arg)?)
        }
        (name, [arg]) if BUILTINS.contains(&name) => {
            let name = if name == "ln" { "log" } else { name };
            Some(format!("{name}({})", print(c, arg)?))
        }
        _ => None,
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
            assert_eq!(print(&c, &tree).as_deref(), Some(expected), "{source}");
        }
    }

    #[test]
    fn translates_a_solve_with_the_equations_variable() {
        let c = Construction::new();
        let tree = parse("Solve(y = 1)", true).unwrap();
        assert_eq!(
            translate(&c, &tree),
            Some(giac::row_solve("Solve", "y=1", "y"))
        );
    }

    #[test]
    fn cas_cell_refuses_unknown_calls_before_dispatch() {
        let c = Construction::new();
        assert!(translate(&c, &parse("unknown(1)", true).unwrap()).is_none());
    }
}
