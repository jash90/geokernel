//! Turns the items of a construction into lines of the script language.
//!
//! The items arrive in construction order. Free objects (a point, a vector,
//! a number) become `A = (x, y)` and `r = 3` with the exact value of the
//! element; a command becomes `label = Name(args)`, or
//! `{label, label} = Name(args)` when it has several outputs. Each line is
//! executed on a scratch [`Construction`] before it is accepted, so whatever
//! the kernel cannot do is skipped with the kernel's message.

use crate::xml::{Document, Element, Item};
use crate::{ImportError, Imported, Mismatch, Skipped};
use geo_kernel::script::{exec, is_vector_label, multi_output_count};
use geo_kernel::{Construction, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The longest text a reason may quote from the file, in characters.
const QUOTE_CHARS: usize = 60;

/// The longest label or number literal the importer passes into a script
/// line (anything longer is not a label the script can use).
/// The longest reason kept for a skipped object, in characters.
const MAX_REASON_CHARS: usize = 300;

const MAX_TOKEN_BYTES: usize = 256;

/// `text`, cut to [`QUOTE_CHARS`] characters (with `...`) so that a reason
/// never grows with the input it talks about.
pub fn quote(text: &str) -> String {
    match text.char_indices().nth(QUOTE_CHARS) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text.to_string(),
    }
}

/// Runs the whole conversion.
pub fn convert(doc: Document) -> Result<Imported, ImportError> {
    let mut c = Converter::default();
    let mut flags = Flags::default();
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
            Item::Element(el) => {
                flags.count(&el);
                c.element(*el)?;
            }
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
    if doc.constructions > 1 {
        out.notes.push(format!(
            "the file has {} construction sections; their elements are appended one after \
             the other, as GeoGebra's reader does",
            doc.constructions
        ));
    }
    flags.notes(&mut out.notes);
    Ok(out)
}

/// Properties of elements that no value depends on and the kernel has no
/// place for; only counted.
#[derive(Default)]
struct Flags {
    fixed: usize,
    auxiliary: usize,
    conditional: usize,
    start_points: usize,
}

impl Flags {
    fn count(&mut self, el: &Element) {
        self.fixed += el.fixed as usize;
        self.auxiliary += el.auxiliary as usize;
        self.conditional += el.conditional as usize;
        self.start_points += el.start_point as usize;
    }

    fn notes(&self, notes: &mut Vec<String>) {
        for (n, what) in [
            (self.fixed, "fixed (the kernel has no fixed flag)"),
            (self.auxiliary, "auxiliary objects"),
            (
                self.conditional,
                "objects with a condition for showing them (not used)",
            ),
            (
                self.start_points,
                "vectors with a start point (the kernel draws none)",
            ),
        ] {
            if n > 0 {
                notes.push(format!("the file has {n} {what}"));
            }
        }
    }
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

/// One script line and the labels it has to create.
struct Step {
    line: String,
    labels: Vec<String>,
    /// The kernel chooses the labels of the objects this line makes (a
    /// polygon and its sides, the vertices of a polygon, its angles); the
    /// labels of the file are matched to them afterwards.
    auto: bool,
}

/// Commands whose outputs the kernel labels itself, as GeoGebra does.
fn labels_by_kernel(name: &str, args: &[String]) -> bool {
    matches!(name, "Polygon" | "PolyLine" | "Vertex" | "InteriorAngles")
        || (name == "Angle" && args.len() == 1)
}

impl Converter {
    fn skip(&mut self, label: &str, kind: &str, mut reason: String) {
        // Whatever produced the reason, it stays short.
        if let Some((end, _)) = reason.char_indices().nth(MAX_REASON_CHARS) {
            reason.truncate(end);
            reason.push_str("...");
        }
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
        let steps = match labelled.iter().find(|l| !is_label(l)) {
            Some(bad) => Err(format!("`{}` is not a label the script can use", quote(bad))),
            None => self.steps(name, inputs, outputs),
        };
        match steps {
            Ok(steps) => {
                for step in steps {
                    if step.auto {
                        match self.run_auto(&step)? {
                            Ok(unmatched) => {
                                self.out.script.push(step.line);
                                let reason = format!(
                                    "the kernel does not make an object with this label for `{}`",
                                    step.labels[0]
                                );
                                for label in unmatched {
                                    self.skip(&label, name, reason.clone());
                                }
                            }
                            Err(reason) => {
                                for label in &step.labels {
                                    self.skip(label, name, reason.clone());
                                }
                            }
                        }
                        continue;
                    }
                    match self.run(&step.line, &step.labels)? {
                        Ok(()) => self.out.script.push(step.line),
                        Err(reason) => {
                            for label in &step.labels {
                                self.skip(label, name, reason.clone());
                            }
                        }
                    }
                }
            }
            Err(reason) => {
                for label in labelled {
                    self.skip(label, name, reason.clone());
                }
            }
        }
        Ok(())
    }

    /// Runs a line on the scratch construction. The line is accepted only when
    /// it creates exactly the objects the file has for it. The outer error is
    /// a bug: a line the kernel refused that nevertheless changed it, or
    /// created other objects than it was told to (the forms the importer
    /// writes cannot, and are checked before the run).
    fn run(&mut self, line: &str, expected: &[String]) -> Result<Result<(), String>, ImportError> {
        let before = self.kernel.len();
        let result = exec(&mut self.kernel, line);
        // Exactly the expected labels, and nothing else, were added.
        let created = self.kernel.len() - before;
        let matches = created == expected.len()
            && expected
                .iter()
                .all(|l| self.kernel.id(l).is_some_and(|id| id.index() >= before));
        match result {
            Ok(_) if matches => Ok(Ok(())),
            Ok(_) => Err(ImportError::Internal(format!(
                "`{line}` created {created} objects, expected {expected:?}"
            ))),
            Err(e) if self.kernel.len() == before => Ok(Err(e)),
            Err(e) => Err(ImportError::Internal(format!(
                "`{line}` failed ({e}) after it had created objects"
            ))),
        }
    }

    /// Runs a line whose objects the kernel labels itself, as GeoGebra does.
    /// Every label the file has for them must be one the kernel made; a file
    /// label with no such object (a side the user renamed) is skipped with its
    /// dependents, and everything else stays in the script. The
    /// outer error is a bug.
    fn run_auto(&mut self, step: &Step) -> Result<Result<Vec<String>, String>, ImportError> {
        let before = self.kernel.len();
        match exec(&mut self.kernel, &step.line) {
            Err(e) if self.kernel.len() == before => return Ok(Err(e)),
            Err(e) => {
                return Err(ImportError::Internal(format!(
                    "`{}` failed ({e}) after it had created objects",
                    step.line
                )))
            }
            Ok(_) => {}
        }
        let made: Vec<String> = self
            .kernel
            .ids()
            .skip(before)
            .map(|id| self.kernel.label(id).to_string())
            .collect();
        for label in &made {
            // A name the file uses later for another object cannot be told
            // from this one; the duplicate check of `claim` reports it.
            self.seen.insert(label.clone());
        }
        // GeoGebra lists the outputs of `Polygon(A, B, n)` in another order
        // than the kernel creates them, so only the sets are compared.
        let unmatched = step
            .labels
            .iter()
            .filter(|l| !made.contains(l))
            .cloned()
            .collect();
        Ok(Ok(unmatched))
    }

    /// The script lines of a command, or why there are none.
    fn steps(
        &self,
        name: &str,
        inputs: &[String],
        outputs: &[String],
    ) -> Result<Vec<Step>, String> {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("`{}` is not a command name", quote(name)));
        }
        let mut args = Vec::with_capacity(inputs.len());
        for arg in inputs {
            for token in tokens(arg) {
                if self.skipped.contains_key(token) {
                    return Err(format!("depends on the skipped object `{}`", quote(token)));
                }
            }
            args.push(pass_argument(arg)?);
        }
        // GeoGebra saves `Intersect(a, b)` of a line and a conic (or two
        // conics) without an index and lists both points as outputs; the
        // kernel takes the n-th point as `Intersect(a, b, n)`.
        if name == "Intersect" && args.len() == 2 && outputs.len() >= 2 {
            return Ok(outputs
                .iter()
                .enumerate()
                .filter(|(_, label)| !label.is_empty())
                .map(|(n, label)| Step {
                    line: format!("{label} = Intersect({}, {}, {})", args[0], args[1], n + 1),
                    labels: vec![label.clone()],
                    auto: false,
                })
                .collect());
        }
        let call = format!("{name}({})", args.join(", "));
        let labelled: Vec<String> = outputs.iter().filter(|l| !l.is_empty()).cloned().collect();
        if outputs.len() > 1 && labels_by_kernel(name, inputs) {
            return Ok(vec![Step {
                line: format!("{} = {call}", labelled[0]),
                labels: labelled,
                auto: true,
            }]);
        }
        let line = match multi_output_count(&self.kernel, &call) {
            // A command that makes several objects: every output gets its label.
            Some(_) => format!("{{{}}} = {call}", outputs.join(", ")),
            None if outputs.len() == 1 => format!("{} = {call}", outputs[0]),
            None => {
                return Err(format!(
                    "the file lists {} outputs, the kernel makes one object for this command",
                    outputs.len()
                ))
            }
        };
        Ok(vec![Step {
            line,
            labels: labelled,
            auto: false,
        }])
    }

    fn element(&mut self, el: Element) -> Result<(), ImportError> {
        if self.produced.contains(&el.label) {
            // The result of a command: its value is the kernel's business.
            self.expressions.remove(&el.label);
            if let Some(id) = self.kernel.id(&el.label) {
                if let Some((saved, computed)) = cross_check(&el, &self.kernel.value(id)) {
                    self.out.mismatched.push(Mismatch {
                        label: el.label.clone(),
                        saved,
                        computed,
                    });
                }
                if !el.shown {
                    self.out.hidden.push(el.label);
                }
            }
            return Ok(());
        }
        self.claim(&el.label)?;
        let expression = self.expressions.remove(&el.label);
        let (label, kind) = (el.label.clone(), el.kind.clone());
        match self.free_object(&el, expression.as_deref())? {
            Ok(line) => match self.run(&line, std::slice::from_ref(&label))? {
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
            // A constant expression (`(1/2, 3)`, `2^10`) is an independent
            // object for GeoGebra, whose value is the one the element saved.
            if !is_literal(exp) && !is_constant_expression(exp) {
                return Ok(expression_line(exp));
            }
        }
        let label = el.label.as_str();
        if !is_label(label) {
            return Ok(Err(format!("`{}` is not a label the script can use", quote(label))));
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
        "the object is defined by the expression `{}`; the kernel has no expressions yet",
        quote(exp)
    ))
}

/// An argument of a command as a script argument. A label or a number goes
/// through unchanged; anything else is an expression (`(1, 2)`, `2 * r`,
/// `{A, B}`), which the kernel cannot read yet. Like [`expression_line`] this
/// is the place to add one.
pub fn pass_argument(arg: &str) -> Result<String, String> {
    let arg = arg.trim();
    let short = arg.len() <= MAX_TOKEN_BYTES;
    if short && (is_label(arg) || arg.parse::<f64>().is_ok_and(f64::is_finite)) {
        Ok(arg.to_string())
    } else {
        Err(format!(
            "the argument `{}` is an expression; the kernel has no expressions yet",
            quote(arg)
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

/// An expression with no object in it, only numbers and operators:
/// digits, `.`, `,`, `+ - * / ^`, parentheses, white space, and an exponent
/// `E` or `e` right after a digit or `.`. GeoGebra treats it as an
/// independent object (`isIndependent = !n.any(isDynamicGeoElement)`), so the
/// importer reads the value from the element, as for a literal. Any
/// identifier (`pi`, `sqrt`, a label) makes it an ordinary expression.
pub fn is_constant_expression(s: &str) -> bool {
    let mut previous = ' ';
    let mut digits = false;
    for ch in s.chars() {
        let ok = match ch {
            '0'..='9' => {
                digits = true;
                true
            }
            '.' | ',' | '+' | '-' | '*' | '/' | '^' | '(' | ')' => true,
            c if c.is_whitespace() => true,
            'E' | 'e' => previous.is_ascii_digit() || previous == '.',
            _ => false,
        };
        if !ok {
            return false;
        }
        previous = ch;
    }
    digits
}

/// A label: letters, digits, `_`, `'` and an index `_{...}` whose inside has
/// letters, digits, `_` and `'` (the same rule as the kernel's output labels).
fn is_label(s: &str) -> bool {
    let (base, index) = match s.strip_suffix('}').and_then(|t| t.split_once("_{")) {
        Some((base, inner)) => (base, Some(inner)),
        None => (s, None),
    };
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let ok_index = index.is_none_or(|d| !d.is_empty() && d.chars().all(word));
    let mut chars = base.chars();
    let first = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    ok_index && first && chars.all(word)
}

/// The label-like words of an argument.
fn tokens(arg: &str) -> impl Iterator<Item = &str> {
    arg.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '\'' | '{' | '}')))
        .filter(|t| !t.is_empty())
}

const TOLERANCE: f64 = 1e-9;

/// `|p - q| <= 1e-9 * max(1, |p|, |q|)`, the comparison of the golden files.
fn close(p: f64, q: f64) -> bool {
    (p - q).abs() <= TOLERANCE * 1.0_f64.max(p.abs()).max(q.abs())
}

fn num(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Three numbers of `<coords>`, all finite.
fn coords3(el: &Element) -> Option<[f64; 3]> {
    let [x, y, z] = el.coords.as_ref()?;
    Some([num(x)?, num(y)?, num(z)?])
}

/// Whether the lines `(a, b, c)` and `(sa, sb, sc)` are the same up to a
/// positive factor (the orientation of a line is part of its value).
fn same_line(line: [f64; 3], saved: [f64; 3]) -> bool {
    let (n, sn) = (line[0].hypot(line[1]), saved[0].hypot(saved[1]));
    n > 0.0 && sn > 0.0 && (0..3).all(|i| close(line[i] / n, saved[i] / sn))
}

fn saved_text(el: &Element) -> String {
    let mut parts = Vec::new();
    if let Some([x, y, z]) = &el.coords {
        parts.push(format!("coords ({x}, {y}, {z})"));
    }
    if let Some(m) = &el.matrix {
        parts.push(format!("matrix [{}]", m.join(", ")));
    }
    if let Some(v) = &el.value {
        parts.push(format!("value {v}"));
    }
    parts.join(", ")
}

/// Compares what the kernel computed with what the file saved for the same
/// object. `Some((saved, computed))` for a disagreement, `None` when they
/// agree or when the file saved nothing the kernel value can be compared with.
fn cross_check(el: &Element, value: &Value) -> Option<(String, String)> {
    let agrees: Option<bool> = match *value {
        Value::Point { x, y } => {
            coords3(el).map(|[sx, sy, sz]| sz != 0.0 && close(x, sx / sz) && close(y, sy / sz))
        }
        Value::Vector { x, y } => coords3(el).map(|[sx, sy, _]| close(x, sx) && close(y, sy)),
        Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => {
            coords3(el).map(|saved| same_line([a, b, c], saved))
        }
        Value::Segment { x1, y1, x2, y2 } => coords3(el).map(|saved| {
            if x1 == x2 && y1 == y2 {
                // GeoGebra saves the line of a degenerate segment as (0, 0, 0).
                saved == [0.0, 0.0, 0.0]
            } else {
                same_line([y1 - y2, x2 - x1, x1 * y2 - y1 * x2], saved)
            }
        }),
        Value::Circle { cx, cy, r } => el.matrix.as_ref().and_then(|m| {
            let a: Vec<f64> = m.iter().map(|t| num(t)).collect::<Option<_>>()?;
            // A0 x^2 + A1 y^2 + A2 + 2 A3 x y + 2 A4 x + 2 A5 y = 0
            if a[0] == 0.0 || !close(a[0], a[1]) || a[3].abs() > TOLERANCE * a[0].abs() {
                return Some(false);
            }
            let (sx, sy) = (-a[4] / a[0], -a[5] / a[0]);
            let r2 = sx * sx + sy * sy - a[2] / a[0];
            // The saved radius is the difference of terms of the size of
            // `sx^2 + sy^2`, so its error is relative to those terms, not to
            // the (possibly tiny) radius.
            let scale = 1.0_f64.max(sx * sx + sy * sy).max((a[2] / a[0]).abs());
            Some(close(cx, sx) && close(cy, sy) && (r * r - r2).abs() <= TOLERANCE * scale)
        }),
        Value::Number(n) => el.value.as_deref().and_then(num).map(|s| close(n, s)),
        Value::Bool(b) => el
            .value
            .as_deref()
            .map(|v| v.trim() == if b { "true" } else { "false" }),
        // The kernel has no value. The file disagrees when it saved a defined
        // one: a point at infinity (z = 0) and a line with (a, b) = (0, 0)
        // are undefined for the kernel, as in the golden snapshots.
        Value::Undefined => match el.kind.as_str() {
            "point" => coords3(el).map(|[_, _, z]| z == 0.0),
            "line" | "segment" | "ray" => coords3(el).map(|[a, b, _]| a == 0.0 && b == 0.0),
            "vector" => coords3(el).map(|_| false),
            _ => el.value.as_deref().and_then(num).map(|_| false),
        },
        _ => None,
    };
    (agrees == Some(false)).then(|| (saved_text(el), format!("{value:?}")))
}
