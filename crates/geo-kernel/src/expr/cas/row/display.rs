use super::{CasRow, Expr};
use std::fmt::{self, Write};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Syntax,
    Text,
    #[cfg(feature = "giac")]
    NumericText,
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

#[cfg(feature = "giac")]
pub(super) fn render_numeric(tree: &Expr) -> CasRow {
    CasRow {
        text: Display(tree, Style::NumericText, 0).to_string(),
        latex: format!(r"\mathbf{{{}}}", Display(tree, Style::Latex, 0)),
        exact: None,
    }
}

#[cfg(feature = "giac")]
pub(super) fn rounded(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

struct Display<'a>(&'a Expr, Style, u8);

fn precedence(tree: &Expr) -> u8 {
    match tree {
        Expr::Eq(..) => 0,
        Expr::Bin(op, ..) if super::super::syntax::relation(*op).is_some() => 0,
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
        let parentheses = own < outer && !(latex && matches!(tree, Expr::Bin(b'/', ..)));
        if parentheses {
            f.write_str(if latex { r"\left(" } else { "(" })?;
        }
        match tree {
            #[cfg(feature = "giac")]
            Expr::Num(n, _) if style == Style::NumericText && n.is_finite() => {
                f.write_str(&rounded(*n))?;
            }
            Expr::Num(n, text) => match text {
                Some(text) => f.write_str(text)?,
                None if n.is_finite() => write!(f, "{n}")?,
                None => f.write_str("?")?,
            },
            Expr::Name(n) if style != Style::Syntax && arbitrary_constant(n) => {
                write!(f, "c_{{{}}}", &n[2..])?;
            }
            Expr::Name(n) => f.write_str(match (n.as_str(), style) {
                ("pi" | "π", Style::Latex) => r"\pi ",
                ("pi", Style::Text) => "π",
                ("ί", Style::Latex) => "i",
                ("ℯ", Style::Latex) => r"\textit{e}",
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
                    _ => {
                        let exponent = syntax(b);
                        if let Some(power) = crate::expr::commands::text::superscript(&exponent) {
                            f.write_str(&power)?;
                        } else {
                            write!(f, "^({})", child(b, 0))?;
                        }
                    }
                }
            }
            Expr::Bin(op, a, b) if super::super::syntax::relation(*op).is_some() => {
                let relation = super::super::syntax::relation(*op).expect("relation");
                write!(f, "{} {relation} {}", child(a, 1), child(b, 1))?;
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
                    (b'*', _) if coefficient_product(a, b) => "",
                    (b'*', Style::Text)
                        if matches!((&**a, &**b), (Expr::Num(..), Expr::Num(..))) =>
                    {
                        " * "
                    }
                    _ => " ",
                };
                let left = child(a, own + u8::from(*op == b'/'));
                let right_precedence = if *op == b'*'
                    && matches!(&**a, Expr::Name(n) if n == "ί")
                    && matches!(&**b, Expr::Bin(b'/', ..))
                {
                    own
                } else {
                    own + 1
                };
                write!(f, "{left}{separator}{}", child(b, right_precedence))?;
            }
            Expr::Call(name, args) => {
                if let ("sqrt", [arg], Style::Latex) = (name.as_str(), args.as_slice(), style) {
                    write!(f, r"\sqrt{{{}}}", child(arg, 0))?;
                } else if let ("abs", [arg], Style::Latex) = (name.as_str(), args.as_slice(), style)
                {
                    write!(f, r"\left|{}\right|", child(arg, 0))?;
                } else if let ("!", [arg]) = (name.as_str(), args.as_slice()) {
                    write!(f, "{}!", child(arg, own))?;
                } else if latex && matches!(name.as_str(), "sin" | "cos" | "tan" | "ln" | "log") {
                    write!(f, r"\operatorname{{{name}}} \left( ")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i != 0 {
                            f.write_str(r",\;")?;
                        }
                        write!(f, "{}", child(arg, 0))?;
                    }
                    f.write_str(r" \right)")?;
                } else {
                    f.write_str(name)?;
                    sequence(f, args, style, false)?;
                }
            }
            Expr::List(items) => {
                if let Some(width) = latex.then(|| matrix_width(items)).flatten() {
                    matrix(f, items, width)?;
                } else {
                    sequence(f, items, style, true)?;
                }
            }
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
        (_, true) => ("{", ", ", "}"),
        (_, false) => ("(", ", ", ")"),
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

fn arbitrary_constant(name: &str) -> bool {
    name.strip_prefix("c_")
        .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
}

fn matrix_width(items: &[Expr]) -> Option<usize> {
    let Expr::List(first) = items.first()? else {
        return None;
    };
    let width = first.len();
    (width != 0
        && items.iter().all(|row| {
            matches!(row, Expr::List(cells)
            if cells.len() == width && cells.iter().all(|c| !matches!(c, Expr::List(..))))
        }))
    .then_some(width)
}

fn matrix(f: &mut fmt::Formatter<'_>, items: &[Expr], width: usize) -> fmt::Result {
    write!(f, r"\left(\begin{{array}}{{{}}}", "r".repeat(width))?;
    for row in items {
        let Expr::List(cells) = row else {
            unreachable!("matrix has only list rows");
        };
        for (i, cell) in cells.iter().enumerate() {
            if i != 0 {
                f.write_str("&")?;
            }
            write!(f, "{}", Display(cell, Style::Latex, 0))?;
        }
        f.write_str(r"\\")?;
    }
    f.write_str(r"\end{array}\right)")
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
