//! A saved equation's display form, attached after its definition.

use crate::expr::lex::{tokens, Tok};
use crate::kernel::{Construction, EquationForm};
use std::fmt;

const DIRECTIVE_NAME: &str = "$equationStyle";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style<'a> {
    pub label: &'a str,
    pub form: EquationForm,
}

impl Style<'_> {
    pub fn apply(self, c: &mut Construction) -> Result<(), String> {
        let id = super::object(c, self.label)?;
        c.set_equation_form(id, self.form)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

impl fmt::Display for Style<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{DIRECTIVE_NAME}({}, {}",
            self.label,
            self.form.xml_style()
        )?;
        if let EquationForm::Parametric(parameter) = &self.form {
            write!(f, ", {}", super::text::literal(parameter))?;
        }
        write!(f, ")")
    }
}

/// Reads the equation form directive when one is present.
pub fn parse_style(line: &str) -> Option<Result<Style<'_>, String>> {
    if !line.strip_prefix(DIRECTIVE_NAME)?.starts_with('(') {
        return None;
    }
    let read = || {
        let (_, args) = super::split_call(line)?;
        let (label, style, parameter) = match args[..] {
            [label, style, parameter] if style == "parametric" => {
                let mut parsed = tokens(parameter).ok()?;
                let token = parsed.pop()?;
                let Tok::Text(name) = token.tok else {
                    return None;
                };
                if !parsed.is_empty() {
                    return None;
                }
                (label, style, Some(name))
            }
            [label, style] => (label, style, None),
            _ => return None,
        };
        let form = EquationForm::from_xml(style, parameter.as_deref())?;
        (!label.is_empty()).then_some(Style { label, form })
    };
    Some(read().ok_or_else(|| "invalid equation style directive".into()))
}

/// Saved implicit element with no defining expression.
pub(super) fn coefficients(c: &mut Construction, line: &str) -> Option<Result<crate::Id, String>> {
    let args = line
        .strip_prefix("$implicitCoefficients(")?
        .strip_suffix(')')?;
    Some((|| {
        let (label, matrix) = args
            .split_once(',')
            .ok_or("expected a label and coefficient matrix")?;
        let label = label.trim();
        if !super::is_output_label(label) {
            return Err("invalid implicit curve label".into());
        }
        super::check_label(label)?;
        let matrix = matrix
            .trim()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or("invalid implicit coefficient matrix")?
            .trim();
        let mut rows = Vec::new();
        let mut remaining = matrix;
        let size = crate::expr::equation::implicit::MAX_DEGREE as usize + 1;
        for _ in 0..size {
            if remaining.is_empty() {
                break;
            }
            let (row, rest) = remaining
                .strip_prefix('[')
                .and_then(|s| s.split_once(']'))
                .ok_or("invalid implicit coefficient row")?;
            let row = row.trim();
            let values = if row.is_empty() {
                Vec::new()
            } else {
                row.split(',')
                    .take(size + 1)
                    .map(|s| s.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "invalid implicit coefficient")?
            };
            rows.push(values);
            let rest = rest.trim();
            remaining = if rest.is_empty() {
                rest
            } else {
                rest.strip_prefix(',')
                    .ok_or("expected another coefficient row")?
                    .trim()
            };
        }
        if !remaining.is_empty() {
            return Err("implicit coefficient matrix exceeds the degree budget".into());
        }
        let id = c
            .add_implicit_coefficients(label, rows)
            .map_err(|e| e.to_string())?;
        c.remember_construction_source(id, line);
        Ok(id)
    })())
}
