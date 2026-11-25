use super::{CasRow, Expr};
use std::fmt::{self, Write};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Syntax,
    Text,
    Latex,
}

pub(super) fn syntax(tree: &Expr) -> String {
    Display(tree, Style::Syntax, 0).to_string()
}

pub(super) fn render(tree: &Expr) -> CasRow {
    let exact = syntax(tree);
    CasRow {
        text: Display(tree, Style::Text, 0).to_string(),
        latex: format!(r"\mathbf{{{}}}", Display(tree, Style::Latex, 0)),
        exact: (!exact.contains('?') && !has_approximation(tree)).then_some(exact),
    }
}

fn approximate_number(tree: &Expr) -> Option<f64> {
    match tree {
        Expr::Num(n, Some(literal)) if literal.contains(['.', 'e', 'E']) && n.fract() != 0.0 => {
            Some(*n)
        }
        Expr::Neg(value) => approximate_number(value).map(|n| -n),
        _ => None,
    }
}

fn has_approximation(tree: &Expr) -> bool {
    match tree {
        Expr::Num(..) => approximate_number(tree).is_some(),
        Expr::Neg(a) => has_approximation(a),
        Expr::Bin(_, a, b) | Expr::Eq(a, b) => has_approximation(a) || has_approximation(b),
        Expr::Call(_, args) | Expr::List(args) | Expr::Tuple(args) => {
            args.iter().any(has_approximation)
        }
        Expr::Name(_) => false,
    }
}

struct Display<'a>(&'a Expr, Style, u8);

fn precedence(tree: &Expr) -> u8 {
    match tree {
        Expr::Eq(..) => 0,
        Expr::Bin(b'+' | b'-', ..) => 1,
        Expr::Bin(b'*' | b'/', ..) => 2,
        Expr::Neg(_) => 3,
        Expr::Bin(b'^', ..) => 4,
        _ => 5,
    }
}

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(tree, style, outer) = *self;
        if style != Style::Syntax {
            if let Some(value) = approximate_number(tree) {
                let text = crate::expr::commands::text::format_number_with_template(
                    value,
                    style == Style::Latex,
                );
                return f.write_str(&text);
            }
        }
        let child = |e, p| Display(e, style, p);
        let latex = style == Style::Latex;
        let own = precedence(tree);
        let parentheses = own < outer;
        if parentheses {
            f.write_str(if latex { r"\left(" } else { "(" })?;
        }
        match tree {
            Expr::Num(n, text) => match text {
                Some(text) => f.write_str(text)?,
                None if n.is_finite() => write!(f, "{n}")?,
                None => f.write_str("?")?,
            },
            Expr::Name(n) => f.write_str(match (n.as_str(), style) {
                ("pi", Style::Latex) => r"\pi",
                ("pi", Style::Text) => "π",
                ("e", Style::Text) => "ℯ",
                ("inf", Style::Latex) => r"\infty",
                ("inf", Style::Text) => "∞",
                _ => n,
            })?,
            Expr::Neg(a) => write!(f, "-{}", child(a, own))?,
            Expr::Bin(b'/', a, b) if latex => {
                write!(f, r"\frac{{{}}}{{{}}}", child(a, 0), child(b, 0))?;
            }
            Expr::Bin(b'^', a, b) => {
                write!(f, "{}", child(a, own + 1))?;
                match style {
                    Style::Syntax => write!(f, "^{}", child(b, own))?,
                    Style::Latex => write!(f, "^{{{}}}", child(b, 0))?,
                    Style::Text => {
                        let exponent = syntax(b);
                        if let Some(power) = crate::expr::commands::text::superscript(&exponent) {
                            f.write_str(&power)?;
                        } else {
                            write!(f, "^({})", child(b, 0))?;
                        }
                    }
                }
            }
            Expr::Bin(op, a, b) => {
                let separator = match (*op, style) {
                    (_, Style::Syntax) => {
                        write!(f, "{}", child(a, own + u8::from(*op == b'/')))?;
                        f.write_char(*op as char)?;
                        return finish(f, child(b, own + 1), parentheses, latex);
                    }
                    (b'+', _) => " + ",
                    (b'-', _) => " - ",
                    (b'/', _) => " / ",
                    (b'*', Style::Latex)
                        if matches!((&**a, &**b), (Expr::Num(..), Expr::Num(..))) =>
                    {
                        r" \cdot "
                    }
                    (b'*', Style::Latex) => r" \; ",
                    (b'*', Style::Text) if coefficient_product(a, b) => "",
                    (b'*', Style::Text)
                        if matches!((&**a, &**b), (Expr::Num(..), Expr::Num(..))) =>
                    {
                        " * "
                    }
                    _ => " ",
                };
                let left = child(a, own + u8::from(*op == b'/'));
                write!(f, "{left}{separator}{}", child(b, own + 1))?;
            }
            Expr::Call(name, args) => match (name.as_str(), args.as_slice(), style) {
                ("sqrt", [arg], Style::Latex) => write!(f, r"\sqrt{{{}}}", child(arg, 0))?,
                ("abs", [arg], Style::Latex) => write!(f, r"\left|{}\right|", child(arg, 0))?,
                ("!", [arg], _) => write!(f, "{}!", child(arg, own))?,
                _ => {
                    f.write_str(name)?;
                    sequence(f, args, style, false)?;
                }
            },
            Expr::List(items) => sequence(f, items, style, true)?,
            Expr::Tuple(items) => sequence(f, items, style, false)?,
            Expr::Eq(a, b) => {
                let equals = match style {
                    Style::Syntax => "=",
                    Style::Latex if matches!(&**a, Expr::Call(..)) => r"\, = \,",
                    _ => " = ",
                };
                write!(f, "{}{equals}{}", child(a, 1), child(b, 1))?;
            }
        }
        if parentheses {
            f.write_str(if latex { r" \right)" } else { ")" })?;
        }
        Ok(())
    }
}

fn finish(
    f: &mut fmt::Formatter<'_>,
    child: Display<'_>,
    parentheses: bool,
    latex: bool,
) -> fmt::Result {
    write!(f, "{child}")?;
    if parentheses {
        f.write_str(if latex { r" \right)" } else { ")" })?;
    }
    Ok(())
}

fn coefficient_product(a: &Expr, b: &Expr) -> bool {
    let b = match b {
        Expr::Bin(b'^', base, _) => base,
        other => other,
    };
    matches!((a, b), (Expr::Num(..), Expr::Name(_)))
}

fn sequence(f: &mut fmt::Formatter<'_>, items: &[Expr], style: Style, list: bool) -> fmt::Result {
    if list && items.is_empty() && style == Style::Latex {
        return f.write_str(r"\left\{ \right\}");
    }
    let (open, separator, close) = match (style, list) {
        (Style::Latex, true) => (r" \left\{ ", ", ", r" \right\} "),
        (Style::Latex, false) => (r"\left(", r",\;", r" \right)"),
        (Style::Syntax, true) => ("{", ",", "}"),
        (Style::Syntax, false) => ("(", ",", ")"),
        (Style::Text, true) => ("{", ", ", "}"),
        (Style::Text, false) => ("(", ", ", ")"),
    };
    f.write_str(open)?;
    for (index, item) in items.iter().enumerate() {
        if index != 0 {
            f.write_str(separator)?;
        }
        write!(f, "{}", Display(item, style, 0))?;
    }
    f.write_str(close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_solutions_display_approximations_without_an_exact_value() {
        let tree = super::super::parse("{1.414213562373}", false).unwrap();
        let row = render(&tree);
        assert_eq!(row.exact, None);
        assert_eq!(row.text, "{1.41}");
        assert_eq!(row.latex, r"\mathbf{ \left\{ 1.41 \right\} }");
    }
}
