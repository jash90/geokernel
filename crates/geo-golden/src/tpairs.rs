//! Runs the verified upstream t-pairs through `geo-kernel`.
//!
//! `golden/t-pairs/verified.jsonl` records, for every step of the upstream
//! kernel tests (`t`, `tRound`, `add` calls), what GeoGebra returned when
//! the harness replayed the method: the outputs with their labels, the
//! string of the template the upstream helper compares, and the structured
//! object of `golden/SCHEMA.md`. This module replays each method on a fresh
//! construction and compares every step:
//!
//! - by value: the structured object, through the golden comparison rules
//!   (tolerance 1e-9), and
//! - by string: GeoGebra's output string where this crate's GeoGebra-style
//!   text formatting covers the type (numbers, points, vectors, booleans,
//!   segments and lists of these; expressions need an expression printer
//!   that does not exist yet).
//!
//! Steps that do not pass are listed in `t-pairs-gaps.toml` with a category
//! and the exact outputs that disagree. The test fails when an unlisted step
//! fails and when a listed gap starts passing (or changes category), so an
//! entry can neither hide a new disagreement nor go stale.
//!
//! Passing steps are not all one kind, and the report says which: a step
//! whose outputs were compared by value, a literal assignment that only
//! exercises the parser, an input both GeoGebra and the kernel run without
//! creating anything, an input both refuse, and a step of a method whose
//! replay context is incomplete. The compared-value count is the headline.

use crate::compare::{compare_arc, compare_conic, compare_extra, compare_list, compare_sampled};
use crate::model::Expected;
use geo_kernel::{script, Construction, Id, Kind, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A verified t-pair file entry: either the header or one step.
#[derive(Deserialize)]
#[serde(untagged)]
enum Line {
    Header {
        #[allow(dead_code)]
        generator: String,
        #[allow(dead_code)]
        upstream_commit: String,
        #[allow(dead_code)]
        source: String,
    },
    Step(Box<Pair>),
}

/// One replayed `t`/`tRound`/`add` step with GeoGebra's verified answer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// `file:line` of the upstream call, unique across the file.
    pub id: String,
    pub test: Option<String>,
    #[allow(dead_code)]
    pub call: String,
    pub input: String,
    pub template: String,
    pub context_complete: bool,
    #[serde(default)]
    pub expected: Option<Vec<String>>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub matches_expected: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    pub note: Option<String>,
    pub outputs: Vec<Output>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    #[serde(default)]
    pub label: Option<String>,
    /// GeoGebra class name, `GeoPoint`, `GeoList`, ...
    pub class: String,
    /// The string of the template the upstream helper compares.
    pub string: String,
    /// The structured snapshot object, `None` when the schema has no type
    /// for the class (or the object left the plane z = 0).
    #[serde(default)]
    pub object: Option<serde_json::Value>,
    /// Why `object` is `None`, when the harness recorded a reason.
    #[serde(default)]
    #[allow(dead_code)]
    pub unsupported: Option<String>,
}

/// One upstream test method: a fresh construction and its steps in order.
pub struct Method {
    pub file: String,
    pub test: Option<String>,
    pub steps: Vec<Pair>,
}

/// Reads and groups the verified pairs. The methods are the runs of lines
/// with the same file and test name, in file order.
pub fn load(text: &str) -> Result<Vec<Method>, String> {
    let mut methods: Vec<Method> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Line = serde_json::from_str(line)
            .map_err(|e| format!("{}: {e}", line.chars().take(60).collect::<String>()))?;
        let Line::Step(pair) = parsed else {
            continue;
        };
        let (file, _) = pair
            .id
            .split_once(':')
            .ok_or_else(|| format!("id `{}` has no `file:line` form", pair.id))?;
        let file = file.to_string();
        let same = methods
            .last()
            .is_some_and(|m| m.file == file && m.test == pair.test);
        if !same {
            methods.push(Method {
                file,
                test: pair.test.clone(),
                steps: Vec::new(),
            });
        }
        methods.last_mut().unwrap().steps.push(*pair);
    }
    Ok(methods)
}

/// The GeoGebra-style text of a double: the shortest string that reads back
/// as the same number, no trailing `.0`, `-0` folded to `0`, `NaN` and
/// `Infinity` by name. `None` when the value would need the exponent form,
/// which this formatter does not cover yet.
pub fn ggb_double(v: f64) -> Option<String> {
    if v.is_nan() {
        return Some("NaN".into());
    }
    if v.is_infinite() {
        return Some(if v > 0.0 { "Infinity" } else { "-Infinity" }.into());
    }
    let s = format!("{}", v + 0.0);
    if s.contains(['e', 'E']) {
        return None;
    }
    let s = s.strip_suffix(".0").unwrap_or(&s);
    Some(if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    })
}

fn display_number(value: f64, template: &str) -> Option<String> {
    if template != "edit" || !value.is_finite() {
        return ggb_double(value);
    }
    if value.abs() < 0.000005 {
        return Some("0".into());
    }
    let fixed = format!("{value:.5}");
    Some(fixed.trim_end_matches('0').trim_end_matches('.').into())
}

fn display_angle(value: f64, template: &str) -> Option<String> {
    let degrees = value.to_degrees();
    let nearest = (degrees * 1e8).round() / 1e8;
    let degrees = if (nearest - degrees).abs() < 1e-12 {
        nearest
    } else {
        degrees
    };
    let unit = if template == "xml" { "*°" } else { "°" };
    Some(format!("{}{unit}", display_number(degrees, template)?))
}

/// Renders a supported value in the template used to record the pair.
pub fn value_string(c: &Construction, id: Id, template: &str) -> Option<String> {
    let value = c.value(id);
    let number = |v| display_number(v, template);
    let text = |x: f64, y: f64| {
        let (x, y) = (number(x)?, number(y)?);
        Some(format!("({x}, {y})"))
    };
    // `ToComplex` and `ToPolar` only change how the point prints
    // (`GeoPoint.buildValueString`)
    if let Some(mode) = c.coordinate_mode(id) {
        if let (Value::Point { x, y }, true) = (
            &value,
            mode != geo_kernel::algo::point::CoordMode::Cartesian,
        ) {
            return styled_point(*x, *y, mode);
        }
    }
    match (&value, c.kind(id)) {
        (Value::Undefined, kind) => match kind {
            Kind::Number if template == "edit" => Some("?".into()),
            Kind::Number => Some("NaN".into()),
            Kind::Point | Kind::Vector if template == "edit" => Some("(?, ?)".into()),
            Kind::Point | Kind::Vector => Some("(NaN, NaN)".into()),
            // an undefined function or line prints the undefined sign too
            Kind::Boolean | Kind::List | Kind::Function | Kind::Line => Some("?".into()),
            Kind::Text => Some(String::new()),
            _ => None,
        },
        (Value::Number(n), Kind::Angle) => display_angle(*n, template),
        (Value::Number(n), _) => number(*n),
        (Value::Point { x, y }, _) => text(*x, *y),
        (Value::Vector { x, y }, _) => text(*x, *y),
        (Value::Bool(b), _) => Some(b.to_string()),
        (Value::Text, _) => c.text(id).map(str::to_owned),
        (Value::Segment { x1, y1, x2, y2 }, _) => number((x2 - x1).hypot(y2 - y1)),
        (Value::Polygon { area }, _) | (Value::Sector { area }, _) => number(area.abs()),
        (Value::PolyLine { length }, _) | (Value::Arc { length }, _) => number(*length),
        (_, Kind::List) => list_string(c, c.elements(id)?, template, &mut 16_384),
        (
            _,
            Kind::Function
            | Kind::FunctionNVar
            | Kind::Curve
            | Kind::Line
            | Kind::Circle
            | Kind::Conic,
        ) if matches!(template, "xml" | "edit") => c.xml_value_string(id),
        _ => None,
    }
}

/// A point in the coordinate style `ToComplex` or `ToPolar` set
/// (`GeoPoint.buildValueString`): `x + yί` with a signed coefficient, or
/// `(r; θ*°)` with the angle in degrees, as the templates GeoGebra's own
/// tests replay with print.
fn styled_point(x: f64, y: f64, mode: geo_kernel::algo::point::CoordMode) -> Option<String> {
    match mode {
        geo_kernel::algo::point::CoordMode::Complex => {
            let imaginary = '\u{03af}';
            if x != 0.0 {
                let coefficient = if y == -1.0 {
                    "- ".to_string()
                } else if y == 1.0 {
                    "+ ".to_string()
                } else if y < 0.0 {
                    format!("- {}", ggb_double(-y)?)
                } else {
                    format!("+ {}", ggb_double(y)?)
                };
                Some(format!("{} {coefficient}{imaginary}", ggb_double(x)?))
            } else if y == -1.0 {
                Some(format!("-{imaginary}"))
            } else if y == 1.0 {
                Some(format!("{imaginary}"))
            } else {
                Some(format!("{}{imaginary}", ggb_double(y)?))
            }
        }
        geo_kernel::algo::point::CoordMode::Polar => Some(format!(
            "({}; {}*\u{00b0})",
            ggb_double(x.hypot(y))?,
            ggb_double(y.atan2(x).to_degrees())?
        )),
        geo_kernel::algo::point::CoordMode::Cartesian => None,
    }
}

/// A list of numbers (or of such lists) prints `{a, b, c}` in GeoGebra;
/// any other element needs the expression printer.
fn list_string(
    c: &Construction,
    list: geo_kernel::expr::List<'_>,
    template: &str,
    remaining: &mut usize,
) -> Option<String> {
    use geo_kernel::expr::Element;
    *remaining = remaining.checked_sub(list.len())?;
    let mut parts = Vec::new();
    for (index, element) in list.iter().enumerate() {
        match element {
            Element::Value(Kind::Number, Value::Number(n)) => parts.push(ggb_double(n)?),
            Element::Value(Kind::Segment, Value::Segment { x1, y1, x2, y2 }) => {
                parts.push(ggb_double((x2 - x1).hypot(y2 - y1))?);
            }
            Element::Value(Kind::Number, Value::Undefined) => parts.push("NaN".into()),
            Element::Value(Kind::Point | Kind::Vector, Value::Undefined) => {
                parts.push("(NaN, NaN)".into())
            }
            Element::Value(Kind::Boolean, Value::Undefined) => parts.push("?".into()),
            Element::Value(
                Kind::Point | Kind::Vector,
                Value::Point { x, y } | Value::Vector { x, y },
            ) => parts.push(format!("({}, {})", ggb_double(x)?, ggb_double(y)?)),
            Element::Value(Kind::Boolean, Value::Bool(b)) => parts.push(b.to_string()),
            Element::List(inner) => parts.push(list_string(c, inner, template, remaining)?),
            Element::Value(Kind::Function, _) if template == "xml" => {
                let text = if let Some(id) = list.function_at(index) {
                    value_string(c, id, template)?
                } else {
                    c.xml_inline_polynomial(&list.symbolic_at(index)?)?
                };
                parts.push(text);
            }
            Element::Text(s) => parts.push(format!("\"{s}\"")),
            _ => return None,
        }
    }
    Some(format!("{{{}}}", parts.join(", ")))
}

/// The name of the command an input calls, for filtering and per-command
/// summaries: the first capitalised call in the input, in either argument
/// form (`Midpoint(A, B)`, GeoGebra's older `Length[f]`), or `none` when
/// the input is an expression or an assignment of one.
pub fn command_of(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_uppercase() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            // A call is followed by `(` or `[` (the old form); `E` alone in
            // `1.5E10` is not one.
            if i < bytes.len() && (bytes[i] == b'(' || bytes[i] == b'[') && i - start > 1 {
                return input[start..i].to_string();
            }
            if i == start {
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    "none".to_string()
}

/// Where `=` binds: outside brackets and string literals, and not part of
/// `==`, `<=`, `>=` or `!=`. `None` when the input assigns nothing.
fn top_level_equals(input: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if quote {
            if ch == '"' {
                quote = false;
            }
        } else {
            match ch {
                '"' => quote = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                '=' => {
                    let next = chars.get(i + 1) == Some(&'=');
                    let prev = i > 0 && matches!(chars[i - 1], '<' | '>' | '!' | '=');
                    if depth == 0 && !next && !prev {
                        return Some(i);
                    }
                    if next {
                        i += 1;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// GeoGebra's parser accepts command calls with square brackets (`Length[f]`
/// is `Length(f)`) and the old `label:` form of an assignment, and ignores
/// spaces after commas. The kernel's script syntax has neither, so the
/// runner rewrites all three into the current forms before executing: a `[`
/// that directly follows a name starting with a capital letter (a command;
/// lists keep their indexing brackets) becomes `(` with its matching `]` as
/// `)`, a top-level `:` before the first bracket becomes `=`, and a space
/// after a comma is dropped (outside string literals).
fn normalize_syntax(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut rewritten: Vec<bool> = vec![false; chars.len()];
    let is_name = |c: char| c.is_alphanumeric() || c == '_';
    for (i, &ch) in chars.iter().enumerate() {
        if ch != '[' {
            continue;
        }
        // A command call bracket follows a name that starts with a capital.
        let mut start = i;
        while start > 0 && is_name(chars[start - 1]) {
            start -= 1;
        }
        // A command's name carries an upper-case letter (`Vertex`,
        // `nCr`); a lower-case name before `[` is an old-style list index
        // (`l[2]`), which the parentheses form reads the same way.
        if start == i || !chars[start..i].iter().any(|c| c.is_ascii_uppercase()) {
            continue;
        }
        // Find the matching `]`, counting brackets only: parentheses are
        // already balanced inside.
        let mut depth = 0usize;
        let mut close = None;
        for (j, &cj) in chars.iter().enumerate().skip(i) {
            if cj == '[' {
                depth += 1;
            } else if cj == ']' {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
        }
        if let Some(close) = close {
            rewritten[i] = true;
            rewritten[close] = true;
        }
    }
    let mut quote = false;
    let mut skip_space = false;
    for (i, &ch) in chars.iter().enumerate() {
        if quote {
            out.push(ch);
            quote = ch != '"';
            continue;
        }
        if skip_space && ch.is_whitespace() {
            continue;
        }
        skip_space = false;
        match ch {
            '"' => {
                quote = true;
                out.push(ch);
            }
            '[' | ']' if rewritten[i] => out.push(if ch == '[' { '(' } else { ')' }),
            ',' => {
                out.push(ch);
                skip_space = true;
            }
            ':' => {
                // `label:input` is `label = input`: everything before the
                // colon is the label (names and spaces, no brackets yet).
                let label = !out.is_empty()
                    && out.chars().any(is_name)
                    && out.chars().all(|c| is_name(c) || c.is_whitespace());
                if !label || chars.get(i + 1) != Some(&'=') {
                    out.push(if label { '=' } else { ch });
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// The label prefix the runner gives results GeoGebra left unnamed, so an
/// input that assigns nothing can still run. No GeoGebra label looks like
/// this, and the step's line number keeps two probe steps of one method
/// apart.
const PROBE: &str = "tpairs_probe";

/// How the runner ran one pair's input.
#[derive(Debug)]
pub enum Exec {
    /// The input as written (it assigns) or rewritten with the labels
    /// GeoGebra gave its outputs.
    Script(Result<Option<Id>, String>),
    /// GeoGebra's replay named no result of this step (or the step has no
    /// result at all), so the input ran under probe labels: the recorded
    /// unlabelled outputs are compared against them by value. `probes`
    /// maps output indexes to their probe labels; an unlabelled output of
    /// a `Script` run has no probe and cannot be compared.
    Probed {
        result: Result<Option<Id>, String>,
        probes: Vec<(usize, String)>,
    },
    /// The runner cannot express the input at all: a statement about the
    /// runner, never about the kernel.
    Refused(&'static str),
}

impl Exec {
    /// The kernel's refusal, when the run produced one.
    fn error(&self) -> Option<&str> {
        match self {
            Exec::Script(Err(e)) | Exec::Probed { result: Err(e), .. } => Some(e),
            _ => None,
        }
    }

    /// Whether the run created an object.
    fn created(&self) -> bool {
        matches!(
            self,
            Exec::Script(Ok(Some(_)))
                | Exec::Probed {
                    result: Ok(Some(_)),
                    ..
                }
        )
    }

    /// The probe label recorded for output `i`, if any.
    fn probe_label(&self, i: usize) -> Option<&str> {
        match self {
            Exec::Probed { probes, .. } => probes
                .iter()
                .find(|(j, _)| *j == i)
                .map(|(_, l)| l.as_str()),
            _ => None,
        }
    }
}

/// Runs one input the way the upstream helper does. GeoGebra's helper
/// evaluates the input and labels the results itself, so an input without
/// an assignment is rewritten with the labels GeoGebra gave its outputs;
/// results GeoGebra left unlabelled run under probe labels instead of
/// failing as a runner error, and an input with no recorded result at all
/// runs under one probe label just to ask whether the kernel runs it.
fn exec_step(c: &mut Construction, pair: &Pair) -> Exec {
    let input = normalize_syntax(&pair.input);
    if input.trim().is_empty() {
        return Exec::Refused("the input is empty");
    }
    if top_level_equals(&input).is_some() {
        return Exec::Script(script::exec(c, &input));
    }
    if let Some(id) = c.id(input.trim()) {
        if matches!(pair.outputs.as_slice(), [out] if out.label.as_deref() == Some(c.label(id))) {
            return Exec::Script(Ok(Some(id)));
        }
    }
    let line = pair.id.rsplit(':').next().unwrap_or("0");
    if pair.outputs.is_empty() {
        return Exec::Probed {
            result: script::exec(c, &format!("{PROBE}_{line} = {input}")),
            probes: Vec::new(),
        };
    }
    let mut labels: Vec<String> = Vec::with_capacity(pair.outputs.len());
    let mut probes = Vec::new();
    for (i, out) in pair.outputs.iter().enumerate() {
        match out.label.as_deref() {
            Some(label)
                if !(c.id(label).is_some()
                    && matches!(command_of(&input).as_str(), "Curvature" | "CurvatureVector")) =>
            {
                labels.push(label.to_string())
            }
            _ => {
                let probe = format!("{PROBE}_{line}_{i}");
                probes.push((i, probe.clone()));
                labels.push(probe);
            }
        }
    }
    let rewritten = if labels.len() == 1 {
        format!("{} = {}", labels[0], input)
    } else {
        format!("{{{}}} = {}", labels.join(", "), input)
    };
    if probes.is_empty() {
        Exec::Script(script::exec(c, &rewritten))
    } else {
        Exec::Probed {
            result: script::exec(c, &rewritten),
            probes,
        }
    }
}

/// Why a step cannot pass yet. The categories of `t-pairs-gaps.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    /// A command the kernel does not know.
    #[serde(rename = "unsupported-command")]
    UnsupportedCommand,
    /// A known command in a form the kernel refuses or cannot compute.
    #[serde(rename = "unsupported-form")]
    UnsupportedForm,
    /// Needs GeoGebra's CAS.
    #[serde(rename = "cas")]
    Cas,
    /// A command that changes the app or the view (SetValue, ZoomIn, ...),
    /// not the construction.
    #[serde(rename = "app-command")]
    AppCommand,
    /// The values agree but GeoGebra's string needs formatting this crate
    /// has no GeoGebra-compatible counterpart for yet.
    #[serde(rename = "text-formatting")]
    TextFormatting,
    /// The step leaves the plane z = 0 or uses a 3D-only class.
    #[serde(rename = "3d")]
    ThreeD,
    /// The kernel computes something else than GeoGebra for a supported
    /// command: a bug to fix, the reason the lane exists.
    #[serde(rename = "kernel-mismatch")]
    KernelMismatch,
    /// The recorded GeoGebra value comes from the harness without a CAS
    /// backend (its dummy CAS answers nothing), while the upstream test
    /// expects a mocked CAS value; the kernel's own answer is the real one.
    #[serde(rename = "dummy-cas-expectation")]
    DummyCasExpectation,
    /// The method does something between the steps that the replay cannot
    /// (a move, a settings change, Java-side state), so later steps may
    /// legitimately differ.
    #[serde(rename = "incomplete-context")]
    IncompleteContext,
    /// The step assigns to a label that exists: a redefine, which the
    /// kernel's `add` refuses.
    #[serde(rename = "redefine")]
    Redefine,
    /// A GeoGebra class the golden schema has no type for (text, locus,
    /// implicit curve, ...).
    #[serde(rename = "unsupported-type")]
    UnsupportedType,
    /// GeoGebra refuses the input and the kernel accepts it, or the other
    /// way round in a way the categories above do not explain.
    #[serde(rename = "error-behaviour")]
    ErrorBehaviour,
    /// An output GeoGebra left without a label.
    #[serde(rename = "unlabelled")]
    Unlabelled,
    /// The step fails only because an earlier step of its method already
    /// failed here and the object this input names was never created. The
    /// entry points at that root step; fixing it is the lane's work, not
    /// this input's.
    #[serde(rename = "cascade")]
    Cascade,
    /// The runner itself cannot express the input (nothing recorded names a
    /// result it could run it under). A statement about the runner, not
    /// about the kernel.
    #[serde(rename = "runner-limit")]
    RunnerLimit,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::UnsupportedCommand => "unsupported-command",
            Category::UnsupportedForm => "unsupported-form",
            Category::Cas => "cas",
            Category::AppCommand => "app-command",
            Category::TextFormatting => "text-formatting",
            Category::ThreeD => "3d",
            Category::KernelMismatch => "kernel-mismatch",
            Category::DummyCasExpectation => "dummy-cas-expectation",
            Category::IncompleteContext => "incomplete-context",
            Category::Redefine => "redefine",
            Category::UnsupportedType => "unsupported-type",
            Category::ErrorBehaviour => "error-behaviour",
            Category::Unlabelled => "unlabelled",
            Category::Cascade => "cascade",
            Category::RunnerLimit => "runner-limit",
        }
    }
}

/// One output (or the step itself) that does not agree, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Disagreement {
    /// Output label, or `step` for the whole input.
    pub label: String,
    pub why: String,
}

/// How a passing step passed. The passes of a run are reported by kind:
/// only [`PassKind::ComparedValue`] says the kernel computed what GeoGebra
/// computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassKind {
    /// The outputs were compared by value through the golden rules and the
    /// step exercised the kernel beyond reading a literal.
    ComparedValue,
    /// A literal assignment (`A = (1, 2)`, `r = 5`, `t = {1, 2}`): agreeing
    /// says the parser read the literal, nothing more.
    ParseOnly,
    /// GeoGebra's replay and the kernel agree the input creates no output;
    /// that zero is exactly what the upstream helper asserted.
    NoOutput,
    /// GeoGebra refuses the input and so does the kernel: agreement about
    /// the error class (GeoGebra's own class is recorded with the pair;
    /// the kernel has no classes, so the refusal itself is what matches).
    ErrorClass,
    /// The step agrees, but its method's replay context is incomplete, so
    /// the agreement may be coincidence.
    ContextIncomplete,
}

impl PassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PassKind::ComparedValue => "compared-value",
            PassKind::ParseOnly => "parse-only",
            PassKind::NoOutput => "no-output",
            PassKind::ErrorClass => "error-class",
            PassKind::ContextIncomplete => "context-incomplete",
        }
    }
}

/// What running one pair produced.
#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub id: String,
    pub command: String,
    pub disagreements: Vec<Disagreement>,
    /// The category the failures suggest, `None` when the step passes.
    pub category: Option<Category>,
    /// How the step passed, `None` when it is a gap.
    pub pass: Option<PassKind>,
}

/// Compares one output of a step against its recorded object and string,
/// by the label GeoGebra gave it.
fn compare_output(
    c: &Construction,
    out: &Output,
    values_only: bool,
    template: &str,
) -> Vec<Disagreement> {
    let mut bad = Vec::new();
    let Some(label) = out.label.as_deref() else {
        bad.push(Disagreement {
            label: "step".into(),
            why: format!("GeoGebra left the output unlabelled (`{}`)", out.class),
        });
        return bad;
    };
    let Some(id) = c.id(label) else {
        bad.push(Disagreement {
            label: label.into(),
            why: format!(
                "GeoGebra creates a `{}` here, the kernel has no object with this label",
                out.class
            ),
        });
        return bad;
    };
    compare_at(c, out, label, id, values_only, template)
}

/// [`compare_output`], against a result the runner ran under a probe label.
fn compare_probed(
    c: &Construction,
    out: &Output,
    probe: &str,
    values_only: bool,
    template: &str,
) -> Vec<Disagreement> {
    match c.id(probe) {
        Some(id) => compare_at(c, out, probe, id, values_only, template),
        None => vec![Disagreement {
            label: probe.into(),
            why: format!(
                "the kernel names no result for the unlabelled `{}` (probe `{probe}` \
                 created no object)",
                out.class
            ),
        }],
    }
}

fn legacy_implicit(c: &Construction, id: Id, equation: &str) -> Option<Result<(), String>> {
    if c.equation_form(id) != Some(&geo_kernel::kernel::EquationForm::Implicit) {
        return None;
    }
    let actual = c.implicit_coefficients(id)?;
    let (lhs, rhs) = equation.split_once('=')?;
    let expected =
        script::load(&format!("f(x,y) = ({lhs}) - ({rhs})\nc = ImplicitCurve(f)")).ok()?;
    let expected = expected.implicit_coefficients(expected.id("c")?)?;
    Some(
        if crate::compare::implicit_matrices_agree(expected, actual) {
            Ok(())
        } else {
            Err(format!(
                "implicit coefficients {actual:?} do not match {expected:?}"
            ))
        },
    )
}

/// Compares the recorded output against the kernel's object `id`, printed
/// as `label` in messages.
fn compare_at(
    c: &Construction,
    out: &Output,
    label: &str,
    id: Id,
    values_only: bool,
    template: &str,
) -> Vec<Disagreement> {
    let mut bad = Vec::new();
    let Some(raw) = out.object.as_ref() else {
        // Older pairs recorded generated implicit polynomials as equation
        // strings. Recover their numeric coefficients for the same comparison
        // used by the structured snapshots.
        if out.class == "GeoImplicitCurve" {
            if let Some(result) = legacy_implicit(c, id, &out.string) {
                if let Err(why) = result {
                    bad.push(Disagreement {
                        label: label.into(),
                        why,
                    });
                }
                return bad;
            }
        }
        // The schema has no type for this class; only a 3D object that left
        // the plane (or a 3D-only class) keeps `object` empty, every other
        // class is simply not supported by the schema.
        let category_note = if out.class.ends_with("3D") || out.class == "GeoPolyhedron" {
            "3d"
        } else {
            "unsupported-type"
        };
        bad.push(Disagreement {
            label: label.into(),
            why: format!(
                "class `{}` has no schema object ({category_note}); string `{}`",
                out.class, out.string
            ),
        });
        return bad;
    };
    let expected: Expected = crate::model::expected_from_json(raw)
        .unwrap_or_else(|e| panic!("verified pair output of `{label}` does not parse: {e}"));
    // The value, through the golden comparison rules.
    let actual = c.value(id);
    let kind = c.kind(id);
    // ±Infinity is recorded as an undefined number (the schema has no
    // infinity), which drops the sign; the string keeps it. It is a value,
    // not an undefined one: the kernel must hold the very same infinity.
    if let Some(inf) = recorded_infinity(out, &expected) {
        if !matches!(actual, Value::Number(n) if n == inf) {
            bad.push(Disagreement {
                label: label.into(),
                why: format!(
                    "{actual:?} does not match `{}` (GeoGebra records the value {}, \
                     not an undefined number)",
                    out.string, inf
                ),
            });
            return bad;
        }
    } else {
        let value_error = if matches!(kind, Kind::Arc | Kind::Sector) {
            compare_arc(&expected, kind, &actual, c.arc(id).as_ref()).err()
        } else if kind == Kind::Conic {
            compare_conic(&expected, kind, &actual, c.conic(id).as_ref())
                .0
                .err()
        } else if kind == Kind::List {
            compare_list(c, &expected, kind, c.elements(id)).err()
        } else if matches!(
            kind,
            Kind::Text | Kind::FunctionNVar | Kind::Curve | Kind::ImplicitCurve
        ) {
            compare_extra(c, id, &expected).err()
        } else {
            let vertices: Vec<_> = c.vertices(id).collect();
            let sample = |x| c.function_value(id, x);
            compare_sampled(&expected, kind, &actual, &vertices, Some(&sample)).err()
        };
        if let Some(why) = value_error {
            bad.push(Disagreement {
                label: label.into(),
                why: format!("{actual:?} does not match `{}` ({why})", out.string),
            });
            return bad;
        }
    }
    if values_only {
        return bad;
    }
    // The string, where a GeoGebra-compatible formatter exists.
    match value_string(c, id, template) {
        Some(text) if text != out.string => bad.push(Disagreement {
            label: label.into(),
            why: format!("string `{text}`, GeoGebra prints `{}`", out.string),
        }),
        Some(_) => {}
        None => bad.push(Disagreement {
            label: label.into(),
            why: format!(
                "values agree but `{}` of kind {kind} has no GeoGebra-style text yet; \
                 GeoGebra prints `{}`",
                out.class, out.string
            ),
        }),
    }
    bad
}

/// The signed infinity a recorded output holds, when it holds one: the
/// schema writes ±Infinity as an undefined number, so the sign is read
/// back from GeoGebra's string.
fn recorded_infinity(out: &Output, expected: &Expected) -> Option<f64> {
    if expected.kind != Kind::Number || expected.data.is_some() {
        return None;
    }
    match out.string.trim() {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

/// Whether the input assigns a literal (`A = (1, 2)`, `r = 5`, `t = {1, 2}`):
/// numbers, points and vectors of numbers, booleans, and lists of these,
/// nothing symbolic. Such a step can only exercise the parser.
fn assigns_literal(input: &str) -> bool {
    let input = normalize_syntax(input);
    top_level_equals(&input).is_some_and(|eq| is_literal(&input[eq + 1..]))
}

/// The literal grammar of [`assigns_literal`].
fn is_literal(s: &str) -> bool {
    let s = s.trim();
    if matches!(s, "true" | "false") {
        return true;
    }
    let (open, close) = match s.as_bytes().first() {
        Some(b'(') => (b'(', b')'),
        Some(b'{') => (b'{', b'}'),
        _ => return s.parse::<f64>().is_ok(),
    };
    let Some(body) = s
        .strip_prefix(open as char)
        .and_then(|t| t.strip_suffix(close as char))
    else {
        return false;
    };
    let mut depth = 0i32;
    let mut part = String::new();
    for ch in body.chars() {
        match ch {
            '(' | '{' => {
                depth += 1;
                part.push(ch);
            }
            ')' | '}' => {
                depth -= 1;
                part.push(ch);
            }
            ',' | ';' if depth == 0 => {
                if !is_literal(&part) {
                    return false;
                }
                part.clear();
            }
            c => part.push(c),
        }
    }
    // `{}` is the empty list GeoGebra also treats as a literal.
    part.is_empty() || is_literal(&part)
}

/// The bucket a passing step with compared outputs belongs to.
fn pass_kind(pair: &Pair) -> PassKind {
    if !pair.context_complete {
        PassKind::ContextIncomplete
    } else if assigns_literal(&pair.input) {
        PassKind::ParseOnly
    } else {
        PassKind::ComparedValue
    }
}

/// Commands whose answers come from GeoGebra's CAS.
const CAS_COMMANDS: &[&str] = &[
    "CSolve",
    "CSolutions",
    "NSolve",
    "NSolutions",
    "Solve",
    "Solutions",
    "Limit",
    "LimitAbove",
    "LimitBelow",
    "PartialFractions",
    "TrigExpand",
    "TrigCombine",
    "TrigSimplify",
    "Factor",
    "Expand",
    "Simplify",
    "NSolveODE",
    "NDerivative",
    "NumericIntegral",
    "NIntegral",
    "CompleteSquare",
    "Laplace",
    "SolveODE",
    "FurtherCommand",
];

/// Commands that change the app, the view or an object's settings instead
/// of the construction.
const APP_COMMANDS: &[&str] = &[
    "SetValue",
    "SetCoords",
    "SetImage",
    "SetLevelOfDetail",
    "SetCaption",
    "SetColor",
    "SetDynamicColor",
    "SetBackgroundColor",
    "SetLineThickness",
    "SetPointSize",
    "SetPointStyle",
    "SetLineStyle",
    "SetFixed",
    "SetVisibleInView",
    "SetLabelMode",
    "ShowLabel",
    "ShowTrace",
    "SetConditionToShowObject",
    "SetConstructionStep",
    "UpdateConstruction",
    "SetPerspective",
    "SetActiveView",
    "CenterView",
    "ZoomIn",
    "ZoomOut",
    "Pan",
    "SetSeed",
    "Rename",
    "Delete",
    "Execute",
    "RunClickScript",
    "SetTooltipMode",
    "SetLayer",
    "SelectObjects",
    "SetViewDirection",
    "StartAnimation",
    "StopAnimation",
    "StartRecord",
    "StartLogging",
    "StopLogging",
    "SetNoise",
    "ShowAxes",
    "ShowGrid",
    "ShowLayer",
    "HideLayer",
    "ExportImage",
    "PlaySound",
    "ToolImage",
    "ReadText",
    "Repeat",
    "SlowPlot",
    "RunUpdateScript",
    "SetAxesRatio",
    "SetDecoration",
    "SetTrace",
    "SetFilling",
    "SetLineOpacity",
    "SetSpinSpeed",
    "FillCells",
    "FillColumn",
    "FillRow",
    "TurtleForward",
    "TurtleBack",
    "TurtleLeft",
    "TurtleRight",
    "TurtleUp",
    "TurtleDown",
];

/// Picks the category for a failed step from how it failed. `prior_failure`
/// is the first earlier step of the same method that failed here, when one
/// did: an input that names an object that step should have created fails
/// only because of it.
fn classify(
    pair: &Pair,
    exec: &Exec,
    bad: &[Disagreement],
    prior_failure: Option<&str>,
) -> Category {
    if let Exec::Refused(_) = exec {
        return Category::RunnerLimit;
    }
    if let Some(e) = exec.error() {
        // A nested call is a form the kernel's syntax does not have, not a
        // missing command.
        if e.contains("a command inside a command is not supported") {
            return Category::UnsupportedForm;
        }
        // A name the input uses is missing. When an earlier step of the
        // method already failed here, that step is the real gap and this
        // one only follows it down.
        if e.contains("unknown object") {
            return match prior_failure {
                Some(_) => Category::Cascade,
                None if !pair.context_complete => Category::IncompleteContext,
                None => Category::ErrorBehaviour,
            };
        }
    }
    if !pair.context_complete {
        return Category::IncompleteContext;
    }
    if bad
        .iter()
        .any(|d| d.why.starts_with("GeoGebra left the output unlabelled"))
    {
        return Category::Unlabelled;
    }
    let command = command_of(&pair.input);
    if let Some(e) = exec.error() {
        if e.contains("already exists") {
            return Category::Redefine;
        }
        if CAS_COMMANDS.contains(&command.as_str()) {
            return Category::Cas;
        }
        if APP_COMMANDS.contains(&command.as_str()) {
            return Category::AppCommand;
        }
        // The old input syntax (`Length[...]`, `g:...`) and expressions the
        // kernel cannot parse are forms, not missing commands.
        if command == "none" || e.contains("unexpected `[`") || e.contains("unexpected `:`") {
            return Category::UnsupportedForm;
        }
        return Category::UnsupportedCommand;
    }
    if pair.error.is_some() {
        return Category::ErrorBehaviour;
    }
    if bad.iter().any(|d| d.why.contains("(3d)")) {
        return Category::ThreeD;
    }
    if bad.iter().any(|d| d.why.contains("(unsupported-type)")) {
        return Category::UnsupportedType;
    }
    let mocked_cas = pair
        .note
        .as_deref()
        .is_some_and(|note| note.contains("mocked CAS"));
    if bad
        .iter()
        .any(|d| d.why.contains("the kernel has no object with this label"))
    {
        return Category::KernelMismatch;
    }
    if bad.iter().any(|d| d.why.contains("does not match")) {
        return if mocked_cas {
            Category::DummyCasExpectation
        } else {
            Category::KernelMismatch
        };
    }
    if bad
        .iter()
        .any(|d| d.why.starts_with("string `") || d.why.contains("no GeoGebra-style text"))
    {
        return Category::TextFormatting;
    }
    Category::KernelMismatch
}

/// Runs one pair on the construction and compares everything recorded.
pub fn run_pair(c: &mut Construction, pair: &Pair, prior_failure: Option<&str>) -> StepOutcome {
    let exec = exec_step(c, pair);
    let command = command_of(&pair.input);
    // The polynomial CAS is checked through structured outputs. Its
    // presentation conventions are documented independently.
    let values_only = matches!(
        command.as_str(),
        "Expand"
            | "Factor"
            | "IFactor"
            | "Simplify"
            | "CompleteSquare"
            | "IsFactored"
            | "Solve"
            | "Solutions"
            | "NSolve"
            | "NSolutions"
            | "NextPrime"
            | "PreviousPrime"
            | "CommonDenominator"
            | "Degree"
            | "Coefficients"
            | "Numerator"
            | "Denominator"
            | "Polynomial"
            | "Factors"
    );
    let mut bad = Vec::new();
    let mut pass = None;
    if let Exec::Refused(why) = &exec {
        bad.push(Disagreement {
            label: "step".into(),
            why: format!("the runner cannot run this input: {why}"),
        });
    } else {
        match exec.error() {
            Some(e) => {
                if pair.error.is_some() {
                    // Both refuse the input. GeoGebra's own refusal is recorded
                    // with the pair (its class, like IllegalStateException);
                    // the kernel has no error classes, so the refusal itself is
                    // the agreement.
                    pass = Some(PassKind::ErrorClass);
                } else if let Some(root) = prior_failure.filter(|_| e.contains("unknown object")) {
                    bad.push(Disagreement {
                        label: "step".into(),
                        why: format!(
                            "the kernel refuses the input: {e}; the method's step `{root}` \
                         fails earlier here, so the object is missing only in its wake"
                        ),
                    });
                } else {
                    bad.push(Disagreement {
                        label: "step".into(),
                        why: format!("the kernel refuses the input: {e}"),
                    });
                }
            }
            None => {
                if pair.error.is_some() {
                    bad.push(Disagreement {
                        label: "step".into(),
                        why: format!(
                            "GeoGebra refuses the input ({}), the kernel accepts it",
                            error_class(pair.error.as_deref().unwrap_or_default())
                        ),
                    });
                } else if pair.outputs.is_empty() {
                    // The upstream helper asserted exactly the number of results
                    // it expected: zero. Passing needs the kernel to create
                    // nothing too.
                    if exec.created() {
                        bad.push(Disagreement {
                            label: "step".into(),
                            why: "the kernel creates an object here, GeoGebra's replay created \
                                  none (the upstream helper asserts the input has no result)"
                                .into(),
                        });
                    } else {
                        pass = Some(PassKind::NoOutput);
                    }
                } else {
                    for (i, out) in pair.outputs.iter().enumerate() {
                        match (&out.label, exec.probe_label(i)) {
                            (_, Some(probe)) => bad.extend(compare_probed(
                                c,
                                out,
                                probe,
                                values_only,
                                &pair.template,
                            )),
                            (Some(_), None) => {
                                bad.extend(compare_output(c, out, values_only, &pair.template))
                            }
                            (None, None) => bad.push(Disagreement {
                                label: "step".into(),
                                why: format!(
                                    "GeoGebra left the output unlabelled (`{}`)",
                                    out.class
                                ),
                            }),
                        }
                    }
                    if bad.is_empty() {
                        pass = Some(pass_kind(pair));
                    }
                }
            }
        }
    }
    let category = if bad.is_empty() {
        None
    } else {
        Some(classify(pair, &exec, &bad, prior_failure))
    };
    StepOutcome {
        id: pair.id.clone(),
        command,
        disagreements: bad,
        category,
        pass,
    }
}

/// The class of a refusal GeoGebra recorded, for messages: the exception
/// kind of `AssertionError: java.lang.IllegalStateException: ...`, without
/// its package.
fn error_class(error: &str) -> &str {
    let named = error
        .split_once(": ")
        .and_then(|(_, rest)| rest.split_once(':'))
        .map(|(kind, _)| kind.trim());
    let class = named.unwrap_or_else(|| error.split(':').next().unwrap_or(error).trim());
    class.rsplit('.').next().unwrap_or(class)
}

/// One gap entry of `t-pairs-gaps.toml`.
#[derive(Serialize, Deserialize)]
struct GapEntry {
    id: String,
    category: Category,
    reason: String,
    mismatches: Vec<String>,
}

/// The loaded gap list: step id to (category, reason, mismatch labels).
pub type Gaps = BTreeMap<String, (Category, String, BTreeMap<String, ()>)>;

/// Parses `t-pairs-gaps.toml`.
pub fn parse_gaps(text: &str) -> Result<Gaps, String> {
    #[derive(Deserialize)]
    struct File {
        #[serde(default)]
        gap: Vec<GapEntry>,
    }
    let file: File = toml::from_str(text).map_err(|e| format!("t-pairs-gaps.toml: {e}"))?;
    let mut gaps = Gaps::new();
    for g in file.gap {
        if g.reason.trim().is_empty() {
            return Err(format!("t-pairs-gaps.toml: `{}` has an empty reason", g.id));
        }
        if g.mismatches.is_empty() {
            return Err(format!("t-pairs-gaps.toml: `{}` lists no mismatches", g.id));
        }
        let labels = g.mismatches.into_iter().map(|l| (l, ())).collect();
        if gaps
            .insert(g.id.clone(), (g.category, g.reason, labels))
            .is_some()
        {
            return Err(format!("t-pairs-gaps.toml: `{}` is listed twice", g.id));
        }
    }
    Ok(gaps)
}

/// Escapes a string for a TOML basic (double-quoted) string.
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Serialises the gap list back to `t-pairs-gaps.toml` form, sorted by id.
pub fn write_gaps(gaps: &BTreeMap<String, (Category, String, BTreeMap<String, ()>)>) -> String {
    let mut out = String::from(
        "# Verified t-pairs the kernel cannot pass yet.\n\
         #\n\
         # Every entry needs the step id (`file:line` in GeoGebra's tests), a\n\
         # category, a non-empty reason and the exact list of disagreeing\n\
         # outputs (their labels, `step` for the whole input):\n\
         #\n\
         # [[gap]]\n\
         # id = \"commands/CommandsTest.java:1234\"\n\
         # category = \"kernel-mismatch\"\n\
         # reason = \"why the kernel differs from GeoGebra\"\n\
         # mismatches = [\"A\", \"B\"]\n\
         #\n\
         # The test fails when a listed step disagrees elsewhere, or agrees\n\
         # where the list says it does not, so an entry can neither hide a\n\
         # new difference nor go stale.\n\n",
    );
    for (id, (category, reason, labels)) in gaps {
        let _ = writeln!(
            out,
            "[[gap]]\nid = \"{}\"\ncategory = \"{}\"\nreason = \"{}\"\nmismatches = [{}]\n",
            toml_escape(id),
            category.as_str(),
            toml_escape(reason),
            labels
                .keys()
                .map(|l| format!("\"{}\"", toml_escape(l)))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_print_like_geogebra() {
        assert_eq!(ggb_double(4.0).unwrap(), "4");
        assert_eq!(ggb_double(0.5).unwrap(), "0.5");
        assert_eq!(ggb_double(67403403358.94).unwrap(), "67403403358.94");
        assert_eq!(ggb_double(-0.0).unwrap(), "0");
        assert_eq!(ggb_double(f64::NAN).unwrap(), "NaN");
        assert_eq!(ggb_double(f64::INFINITY).unwrap(), "Infinity");
        assert_eq!(ggb_double(-f64::INFINITY).unwrap(), "-Infinity");
        // Rust's Display never switches to the exponent form; whether
        // GeoGebra does for such values has to be learned from new data.
        assert_eq!(ggb_double(1e-7).unwrap(), "0.0000001");
    }

    #[test]
    fn commands_are_found() {
        assert_eq!(command_of("Midpoint(A, B)"), "Midpoint");
        assert_eq!(command_of("a = Midpoint(A, B)"), "Midpoint");
        assert_eq!(command_of("{1,2,3}*2"), "none");
        assert_eq!(command_of("Extremum((x+1)^24)"), "Extremum");
        assert_eq!(command_of("1.5E10 + 2"), "none");
        // GeoGebra's older bracket form of a call.
        assert_eq!(command_of("Length[ Segment((1, 0), (0, 0))]"), "Length");
        assert_eq!(command_of("If[a == 1, 2, 3]"), "If");
        assert_eq!(command_of("list1[2]"), "none");
    }

    #[test]
    fn equals_outside_brackets_and_strings() {
        assert_eq!(top_level_equals("a = 2"), Some(2));
        assert!(top_level_equals("If[a == 1, 2, 3]").is_none());
        assert!(top_level_equals("{1,2,3}*2").is_none());
        assert!(top_level_equals("Sqrt(\"a=b\")").is_none());
        assert!(top_level_equals("a <= 2").is_none());
        assert!(top_level_equals("f(x) = x + 1").is_some());
    }

    #[test]
    fn old_syntax_is_rewritten() {
        use super::normalize_syntax;
        assert_eq!(
            normalize_syntax("Length[ Segment((1, 0), (0, 0))]"),
            "Length( Segment((1,0),(0,0)))"
        );
        assert_eq!(normalize_syntax("If[a == 1, 2, 3]"), "If(a == 1,2,3)");
        // Indexing a list keeps its brackets; a call inside brackets too.
        assert_eq!(normalize_syntax("list1[2]"), "list1[2]");
        assert_eq!(normalize_syntax("Element[list1, 2]"), "Element(list1,2)");
        assert_eq!(normalize_syntax("g:Intersect(e1,e2)"), "g=Intersect(e1,e2)");
        // A colon that is not an assignment stays.
        assert_eq!(normalize_syntax("Text(\"a:b\")"), "Text(\"a:b\")");
        assert_eq!(normalize_syntax("Sqrt[2] + 1"), "Sqrt(2) + 1");
    }

    #[test]
    fn a_passing_and_a_failing_pair() {
        let text = concat!(
            "{\"generator\":\"ggb-harness\",\"upstream_commit\":\"c\",\"source\":\"s\"}\n",
            "{\"id\":\"t/T.java:1\",\"test\":\"m\",\"call\":\"t\",\"input\":\"1 + 1\",",
            "\"template\":\"xml\",\"context_complete\":true,\"expected\":[\"2\"],",
            "\"outputs\":[{\"label\":\"a\",\"class\":\"GeoNumeric\",\"string\":\"2\",",
            "\"object\":{\"type\":\"number\",\"defined\":true,\"value\":2.0}}]}\n",
            "{\"id\":\"t/T.java:2\",\"test\":\"m\",\"call\":\"t\",\"input\":\"1 + 2\",",
            "\"template\":\"xml\",\"context_complete\":true,\"expected\":[\"3\"],",
            "\"outputs\":[{\"label\":\"b\",\"class\":\"GeoNumeric\",\"string\":\"4\",",
            "\"object\":{\"type\":\"number\",\"defined\":true,\"value\":4.0}}]}\n"
        );
        let methods = load(text).unwrap();
        assert_eq!(methods.len(), 1);
        let mut c = Construction::new();
        let a = run_pair(&mut c, &methods[0].steps[0], None);
        assert!(a.disagreements.is_empty());
        assert_eq!(a.pass, Some(PassKind::ComparedValue));
        let b = run_pair(&mut c, &methods[0].steps[1], None);
        assert_eq!(b.disagreements.len(), 1);
        assert!(b.disagreements[0].why.contains("does not match"));
        assert_eq!(b.category, Some(Category::KernelMismatch));
        assert_eq!(b.pass, None);
    }

    /// A pair with no outputs passes only when the kernel also creates
    /// nothing: creating an object GeoGebra's replay never made is a gap,
    /// not a pass on the refusal's absence alone.
    #[test]
    fn a_resultless_pair_never_passes_on_a_creation() {
        let text = concat!(
            "{\"id\":\"t/T.java:1\",\"test\":\"m\",\"call\":\"t\",",
            "\"input\":\"s = 1 + 1\",\"template\":\"xml\",",
            "\"context_complete\":true,\"expected\":[],\"outputs\":[]}\n"
        );
        let methods = load(text).unwrap();
        let mut c = Construction::new();
        let out = run_pair(&mut c, &methods[0].steps[0], None);
        assert!(out.pass.is_none());
        assert!(
            out.disagreements[0]
                .why
                .contains("GeoGebra's replay created none"),
            "actual: {:#?}",
            out.disagreements
        );
        assert_eq!(out.category, Some(Category::KernelMismatch));
    }

    /// Both refusing the input is agreement about the error class.
    #[test]
    fn both_refusing_is_an_error_class_pass() {
        let text = concat!(
            "{\"id\":\"t/T.java:9\",\"test\":\"m\",\"call\":\"t\",",
            "\"input\":\"SetValue(f, ?)\",\"template\":\"xml\",",
            "\"context_complete\":true,\"expected\":[],\"outputs\":[],",
            "\"error\":\"AssertionError: java.lang.IllegalStateException: Please check\"}\n"
        );
        let methods = load(text).unwrap();
        let mut c = Construction::new();
        let out = run_pair(&mut c, &methods[0].steps[0], None);
        assert_eq!(out.pass, Some(PassKind::ErrorClass));
        assert!(out.disagreements.is_empty());
    }

    /// A step that fails on a name an earlier failed step should have
    /// created is a cascade of that step, and says which one.
    #[test]
    fn a_missing_name_after_a_failed_step_is_a_cascade() {
        let text = concat!(
            "{\"id\":\"t/T.java:1\",\"test\":\"m\",\"call\":\"t\",",
            "\"input\":\"q = Zorglub((0, 0), 3)\",\"template\":\"xml\",",
            "\"context_complete\":true,\"expected\":[\"{(-3, 0), (3, 0)}\"],",
            "\"outputs\":[{\"label\":\"q\",\"class\":\"GeoList\",\"string\":\"{(-3, 0), (3, 0)}\",",
            "\"object\":{\"type\":\"list\",\"defined\":true,\"elements\":",
            "[{\"type\":\"point\",\"defined\":true,\"x\":-3.0,\"y\":0.0},",
            "{\"type\":\"point\",\"defined\":true,\"x\":3.0,\"y\":0.0}]}}]}\n",
            "{\"id\":\"t/T.java:2\",\"test\":\"m\",\"call\":\"t\",\"input\":\"r = q + 1\",",
            "\"template\":\"xml\",\"context_complete\":true,\"expected\":[\"{-2, 4}\"],",
            "\"outputs\":[{\"label\":\"r\",\"class\":\"GeoList\",\"string\":\"{-2, 4}\",",
            "\"object\":{\"type\":\"list\",\"defined\":true,\"elements\":",
            "[{\"type\":\"number\",\"defined\":true,\"value\":-2.0},",
            "{\"type\":\"number\",\"defined\":true,\"value\":4.0}]}}]}\n"
        );
        let methods = load(text).unwrap();
        let mut c = Construction::new();
        let first = run_pair(&mut c, &methods[0].steps[0], None);
        assert!(first.category.is_some(), "the root step fails");
        let root = first.id.clone();
        let second = run_pair(&mut c, &methods[0].steps[1], Some(&root));
        assert_eq!(second.category, Some(Category::Cascade));
        assert!(second.disagreements[0].why.contains(&root));
    }

    /// ±Infinity is recorded as an undefined number with the sign in the
    /// string: the kernel must hold the value, being undefined is a
    /// mismatch.
    #[test]
    fn infinity_is_a_value_not_undefined() {
        let text = concat!(
            "{\"id\":\"t/T.java:5\",\"test\":\"m\",\"call\":\"t\",\"input\":\"2^1000\",",
            "\"template\":\"xml\",\"context_complete\":true,\"expected\":[\"Infinity\"],",
            "\"outputs\":[{\"label\":\"d\",\"class\":\"GeoNumeric\",\"string\":\"Infinity\",",
            "\"object\":{\"type\":\"number\",\"defined\":false}}]}\n"
        );
        let methods = load(text).unwrap();
        let mut c = Construction::new();
        let out = run_pair(&mut c, &methods[0].steps[0], None);
        let ok = out.pass == Some(PassKind::ComparedValue);
        let mismatch = out.category == Some(Category::KernelMismatch)
            && out.disagreements[0]
                .why
                .contains("GeoGebra records the value");
        assert!(
            ok || mismatch,
            "the kernel either holds infinity or is filed as differing: {out:?}"
        );
    }

    #[test]
    fn literal_assignments_are_recognised() {
        assert!(assigns_literal("A = (1, 2)"));
        assert!(assigns_literal("r = 5"));
        assert!(assigns_literal("t = {1, 2, 3}"));
        assert!(assigns_literal("v = (1; 2)"));
        assert!(assigns_literal("ok = true"));
        assert!(!assigns_literal("a = Midpoint(A, B)"));
        assert!(!assigns_literal("f(x) = x + 1"));
        assert!(!assigns_literal("a: 2x + 3y = 6"));
        assert!(!assigns_literal("{1,2,3} * 2"));
    }

    #[test]
    fn gaps_round_trip() {
        let toml = "[[gap]]\nid = \"a:1\"\ncategory = \"text-formatting\"\n\
                    reason = \"r\"\nmismatches = [\"A\"]\n";
        let gaps = parse_gaps(toml).unwrap();
        assert_eq!(gaps.len(), 1);
        let written = write_gaps(&gaps);
        let again = parse_gaps(&written).unwrap();
        assert_eq!(format!("{gaps:?}"), format!("{again:?}"));
    }
}
