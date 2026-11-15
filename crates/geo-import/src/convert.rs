//! Turns the items of a construction into lines of the script language.
//!
//! The items arrive in construction order. Free objects (a point, a vector,
//! a number) become `A = (x, y)` and `r = 3` with the exact value of the
//! element; a command becomes `label = Name(args)`, or
//! `{label, label} = Name(args)` when it has several outputs. Each line is
//! executed on a scratch [`Construction`] before it is accepted, so whatever
//! the kernel cannot do is skipped with the kernel's message.

use crate::xml::{Document, Element, Item};
use crate::{ImportError, Imported, Skipped};
use geo_kernel::script::{exec, is_vector_label};
use geo_kernel::Construction;
use std::collections::{BTreeMap, BTreeSet};

/// Runs the whole conversion.
pub fn convert(doc: Document) -> Result<Imported, ImportError> {
    let mut c = Converter::default();
    for item in doc.items {
        match item {
            Item::Expression { label, exp } => {
                c.expressions.insert(label, exp);
            }
            Item::Command {
                name,
                inputs,
                outputs,
            } => c.command(&name, &inputs, &outputs)?,
            Item::Element(el) => c.element(el)?,
        }
    }
    let mut out = c.out;
    out.settings = doc.settings;
    if out.settings.continuous {
        out.notes.push(
            "the construction was saved with continuity on; the kernel does not use it, so \
             the choice between two intersection points can differ from what the file shows"
                .into(),
        );
    }
    Ok(out)
}

#[derive(Default)]
struct Converter {
    out: Imported,
    /// The scratch construction every accepted line was run on.
    kernel: Construction,
    /// Labels of objects made by a command, to tell their elements from free ones.
    produced: BTreeSet<String>,
    /// Every label seen so far, to find duplicates.
    seen: BTreeSet<String>,
    /// Labels of skipped objects and why.
    skipped: BTreeMap<String, String>,
    /// `<expression>` texts waiting for their element.
    expressions: BTreeMap<String, String>,
}

impl Converter {
    fn skip(&mut self, label: &str, kind: &str, reason: String) {
        if !label.is_empty() {
            self.skipped.insert(label.to_string(), reason.clone());
        }
        self.out.skipped.push(Skipped {
            label: label.into(),
            kind: kind.into(),
            reason,
        });
    }

    fn claim(&mut self, label: &str) -> Result<(), ImportError> {
        if !label.is_empty() && !self.seen.insert(label.to_string()) {
            return Err(ImportError::DuplicateLabel(label.into()));
        }
        Ok(())
    }

    fn command(
        &mut self,
        name: &str,
        inputs: &[String],
        outputs: &[String],
    ) -> Result<(), ImportError> {
        for label in outputs {
            self.claim(label)?;
            if !label.is_empty() {
                self.produced.insert(label.clone());
            }
        }
        let labelled: Vec<&String> = outputs.iter().filter(|l| !l.is_empty()).collect();
        if labelled.is_empty() {
            // GeoGebra labels an object only once it is defined, and writes
            // no name for one that is not labelled.
            self.skip("", name, "the command has no labelled output".into());
            return Ok(());
        }
        let line = match labelled.iter().find(|l| !is_label(l)) {
            Some(bad) => Err(format!("`{bad}` is not a label the script can use")),
            None => self.command_line(name, inputs, outputs),
        };
        match line {
            Ok(line) => match self.run(&line, &labelled) {
                Ok(()) => self.out.script.push(line),
                Err(reason) => {
                    for label in labelled {
                        self.skip(label, name, reason.clone());
                    }
                }
            },
            Err(reason) => {
                for label in labelled {
                    self.skip(label, name, reason.clone());
                }
            }
        }
        Ok(())
    }

    /// Runs a line on the scratch construction. The line is accepted only when
    /// it creates exactly the objects the file has for it; otherwise the
    /// scratch construction is rebuilt from the lines accepted so far.
    fn run(&mut self, line: &str, expected: &[&String]) -> Result<(), String> {
        let before = self.kernel.len();
        let result = exec(&mut self.kernel, line);
        let created: Vec<&str> = self
            .kernel
            .ids()
            .skip(before)
            .map(|id| self.kernel.label(id))
            .collect();
        let matches =
            created.len() == expected.len() && created.iter().zip(expected).all(|(a, b)| a == b);
        let outcome = match result {
            Ok(_) if matches => return Ok(()),
            Ok(_) => Err(format!(
                "the kernel would create {created:?} for this command, the file has {:?}",
                expected
            )),
            Err(e) => Err(e),
        };
        if self.kernel.len() != before {
            self.kernel = Construction::new();
            for accepted in &self.out.script {
                exec(&mut self.kernel, accepted).expect("an accepted line runs again");
            }
        }
        outcome
    }

    /// The script line of a command, or why there is none.
    fn command_line(
        &self,
        name: &str,
        inputs: &[String],
        outputs: &[String],
    ) -> Result<String, String> {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("`{name}` is not a command name"));
        }
        let mut args = Vec::with_capacity(inputs.len());
        for arg in inputs {
            for token in tokens(arg) {
                if self.skipped.contains_key(token) {
                    return Err(format!("depends on the skipped object `{token}`"));
                }
            }
            args.push(pass_argument(arg)?);
        }
        let call = format!("{name}({})", args.join(", "));
        Ok(if outputs.len() == 1 {
            format!("{} = {call}", outputs[0])
        } else {
            format!("{{{}}} = {call}", outputs.join(", "))
        })
    }

    fn element(&mut self, el: Element) -> Result<(), ImportError> {
        if self.produced.contains(&el.label) {
            // The result of a command: its value is the kernel's business.
            self.expressions.remove(&el.label);
            if !el.shown && self.kernel.id(&el.label).is_some() {
                self.out.hidden.push(el.label);
            }
            return Ok(());
        }
        self.claim(&el.label)?;
        let expression = self.expressions.remove(&el.label);
        let (label, kind) = (el.label.clone(), el.kind.clone());
        match self.free_object(&el, expression.as_deref())? {
            Ok(line) => match self.run(&line, &[&label]) {
                Ok(()) => {
                    self.out.script.push(line);
                    if !el.shown {
                        self.out.hidden.push(label);
                    }
                }
                Err(e) => self.skip(&label, &kind, e),
            },
            Err(reason) => self.skip(&label, &kind, reason),
        }
        Ok(())
    }

    /// The script line of an element that no command made, or why there is
    /// none. The outer `Result` is for a file that is damaged.
    fn free_object(
        &self,
        el: &Element,
        expression: Option<&str>,
    ) -> Result<Result<String, String>, ImportError> {
        if let Some(exp) = expression {
            if !is_literal(exp) {
                return Ok(expression_line(exp));
            }
        }
        let label = el.label.as_str();
        if !is_label(label) {
            return Ok(Err(format!("`{label}` is not a label the script can use")));
        }
        let number = |text: &str| -> Result<f64, ImportError> {
            text.trim()
                .parse::<f64>()
                .map_err(|_| ImportError::BadNumber {
                    label: label.into(),
                    text: text.into(),
                })
        };
        // The line is written with `{}`, which prints the shortest text that
        // reads back as the same f64.
        Ok(match el.kind.as_str() {
            "point" | "vector" => {
                let Some([x, y, z]) = &el.coords else {
                    return Ok(Err("the element has no coordinates".into()));
                };
                let (x, y, z) = (number(x)?, number(y)?, number(z)?);
                if el.kind == "point" {
                    if z == 0.0 {
                        return Ok(Err("a point at infinity".into()));
                    }
                    let (x, y) = if z == 1.0 { (x, y) } else { (x / z, y / z) };
                    if !(x.is_finite() && y.is_finite()) {
                        return Ok(Err("the coordinates are not finite".into()));
                    }
                    Ok(if is_vector_label(label) {
                        format!("{label} = $point({x}, {y})")
                    } else {
                        format!("{label} = ({x}, {y})")
                    })
                } else {
                    if !(x.is_finite() && y.is_finite()) {
                        return Ok(Err("the coordinates are not finite".into()));
                    }
                    Ok(if is_vector_label(label) {
                        format!("{label} = ({x}, {y})")
                    } else {
                        format!("{label} = $vector({x}, {y})")
                    })
                }
            }
            "numeric" => {
                let Some(text) = &el.value else {
                    return Ok(Err("the element has no value".into()));
                };
                let v = number(text)?;
                if v.is_finite() {
                    Ok(format!("{label} = {v}"))
                } else {
                    Err("the value is not finite".into())
                }
            }
            "angle" => Err("a free angle: the kernel has no angle objects".into()),
            "boolean" => {
                Err("a free boolean (a check box): the kernel has no free booleans".into())
            }
            "conic" => Err("a conic that no command made: the kernel only has circles".into()),
            other => Err(format!("the element type `{other}` is not supported")),
        })
    }
}

/// An `<expression>` that defines an object (`a + b`, `2 * r`). The kernel has
/// no expression engine yet; this is where one would plug in, by returning
/// the script line.
pub fn expression_line(exp: &str) -> Result<String, String> {
    Err(format!(
        "the object is defined by the expression `{exp}`; the kernel has no expressions yet"
    ))
}

/// An argument of a command as a script argument. A label or a number goes
/// through unchanged; anything else is an expression (`(1, 2)`, `2 * r`,
/// `{A, B}`), which the kernel cannot read yet. Like [`expression_line`] this
/// is the place to add one.
pub fn pass_argument(arg: &str) -> Result<String, String> {
    let arg = arg.trim();
    if is_label(arg) || arg.parse::<f64>().is_ok_and(f64::is_finite) {
        Ok(arg.to_string())
    } else {
        Err(format!(
            "the argument `{arg}` is an expression; the kernel has no expressions yet"
        ))
    }
}

/// A number written by GeoGebra: `3`, `-0.5`, `1.0E-5`, or a point `(x, y)`.
fn is_literal(s: &str) -> bool {
    let number = |t: &str| t.trim().parse::<f64>().is_ok_and(f64::is_finite);
    let s = s.trim();
    match s.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        Some(inner) => matches!(inner.split_once(','), Some((x, y)) if number(x) && number(y)),
        None => number(s),
    }
}

/// A label: letters, digits, `_`, `'` and the index `_{digits}`.
fn is_label(s: &str) -> bool {
    let (base, index) = match s.strip_suffix('}').and_then(|t| t.split_once("_{")) {
        Some((base, digits)) => (base, Some(digits)),
        None => (s, None),
    };
    let ok_index = index.is_none_or(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    let mut chars = base.chars();
    let first = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    ok_index && first && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '\'')
}

/// The label-like words of an argument.
fn tokens(arg: &str) -> impl Iterator<Item = &str> {
    arg.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '\'' | '{' | '}')))
        .filter(|t| !t.is_empty())
}
