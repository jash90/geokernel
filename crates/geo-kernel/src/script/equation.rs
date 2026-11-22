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
