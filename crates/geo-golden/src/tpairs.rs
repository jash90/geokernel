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
//! fails and when a listed gap starts passing, so an entry can neither hide
//! a new disagreement nor go stale.

use crate::compare::{compare_arc, compare_conic, compare_list, compare_sampled};
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
    #[allow(dead_code)]
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

/// The GeoGebra-style value string of an object, `None` where this crate
/// has no GeoGebra-compatible formatting yet (expressions, equations).
/// Undefined numbers print `NaN`, undefined points and vectors `(NaN, NaN)`
/// and undefined booleans `?`, as GeoGebra's templates do.
pub fn value_string(c: &Construction, id: Id) -> Option<String> {
    let value = c.value(id);
    let text = |x: f64, y: f64| {
        let (x, y) = (ggb_double(x)?, ggb_double(y)?);
        Some(format!("({x}, {y})"))
    };
    match (&value, c.kind(id)) {
        (Value::Undefined, kind) => match kind {
            Kind::Number => Some("NaN".into()),
            Kind::Point | Kind::Vector => Some("(NaN, NaN)".into()),
            Kind::Boolean => Some("?".into()),
            _ => None,
        },
        (Value::Number(n), _) => ggb_double(*n),
        (Value::Point { x, y }, _) => text(*x, *y),
        (Value::Vector { x, y }, _) => text(*x, *y),
        (Value::Bool(b), _) => Some(b.to_string()),
        (Value::Segment { x1, y1, x2, y2 }, _) => ggb_double((x2 - x1).hypot(y2 - y1)),
        (_, Kind::List) => list_string(c.elements(id)?),
        _ => None,
    }
}

/// A list of numbers (or of such lists) prints `{a, b, c}` in GeoGebra;
/// any other element needs the expression printer.
fn list_string(list: geo_kernel::expr::List<'_>) -> Option<String> {
    use geo_kernel::expr::Element;
    let mut parts = Vec::new();
    for element in list.iter() {
        match element {
            Element::Value(Kind::Number, Value::Number(n)) => parts.push(ggb_double(n)?),
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
            Element::List(inner) => parts.push(list_string(inner)?),
            _ => return None,
        }
    }
    Some(format!("{{{}}}", parts.join(", ")))
}

/// The name of the command an input calls, for filtering and per-command
/// summaries: the first capitalised call in the input (`Midpoint(A, B)`),
/// or `none` when the input is an expression or an assignment of one.
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
            // A call is followed by `(`; `E` alone in `1.5E10` is not one.
            if i < bytes.len() && bytes[i] == b'(' && i - start > 1 {
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
        if start == i || !chars[start].is_ascii_uppercase() {
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
                out.push(if label { '=' } else { ch });
            }
            c => out.push(c),
        }
    }
    out
}

/// Runs one input the way the upstream helper does. GeoGebra's helper
/// evaluates the input and labels the results itself, so an input without
/// an assignment is rewritten with the labels GeoGebra gave its outputs.
fn exec_step(c: &mut Construction, pair: &Pair) -> Result<Option<Id>, String> {
    let mut input = normalize_syntax(&pair.input);
    if top_level_equals(&input).is_none() {
        let labels: Vec<&str> = pair
            .outputs
            .iter()
            .map(|o| o.label.as_deref())
            .collect::<Option<Vec<&str>>>()
            .ok_or("unlabelled output")?;
        if labels.is_empty() {
            return Err("the input assigns nothing and creates no object".into());
        }
        input = if labels.len() == 1 {
            format!("{} = {}", labels[0], input)
        } else {
            format!("{{{}}} = {}", labels.join(", "), input)
        };
    }
    script::exec(c, &input)
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
            Category::IncompleteContext => "incomplete-context",
            Category::Redefine => "redefine",
            Category::UnsupportedType => "unsupported-type",
            Category::ErrorBehaviour => "error-behaviour",
            Category::Unlabelled => "unlabelled",
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

/// What running one pair produced.
#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub id: String,
    pub command: String,
    pub disagreements: Vec<Disagreement>,
    /// The category the failures suggest, `None` when the step passes.
    pub category: Option<Category>,
}

/// Compares one output of a step against its recorded object and string.
fn compare_output(c: &Construction, out: &Output) -> Vec<Disagreement> {
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
    let Some(raw) = out.object.as_ref() else {
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
    let value_error = if matches!(kind, Kind::Arc | Kind::Sector) {
        compare_arc(&expected, kind, &actual, c.arc(id).as_ref()).err()
    } else if kind == Kind::Conic {
        compare_conic(&expected, kind, &actual, c.conic(id).as_ref())
            .0
            .err()
    } else if kind == Kind::List {
        compare_list(&expected, kind, c.elements(id)).err()
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
    // The string, where a GeoGebra-compatible formatter exists.
    match value_string(c, id) {
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
];

/// Picks the category for a failed step from how it failed.
fn classify(pair: &Pair, run: &Result<Option<Id>, String>, bad: &[Disagreement]) -> Category {
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
    if let Err(e) = run {
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
    if bad
        .iter()
        .any(|d| d.why.contains("the kernel has no object with this label"))
    {
        return Category::KernelMismatch;
    }
    if bad.iter().any(|d| d.why.contains("does not match")) {
        return Category::KernelMismatch;
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
pub fn run_pair(c: &mut Construction, pair: &Pair) -> StepOutcome {
    let run = exec_step(c, pair);
    let mut bad = Vec::new();
    match (&run, pair.error.as_ref()) {
        (Err(e), _) => bad.push(Disagreement {
            label: "step".into(),
            why: format!("the kernel refuses the input: {e}"),
        }),
        (Ok(_), Some(ggb)) => bad.push(Disagreement {
            label: "step".into(),
            why: format!("GeoGebra refuses the input ({ggb}), the kernel accepts it"),
        }),
        (Ok(_), None) => {
            for out in &pair.outputs {
                bad.extend(compare_output(c, out));
            }
        }
    }
    let category = if bad.is_empty() {
        None
    } else {
        Some(classify(pair, &run, &bad))
    };
    StepOutcome {
        id: pair.id.clone(),
        command: command_of(&pair.input),
        disagreements: bad,
        category,
    }
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
        let a = run_pair(&mut c, &methods[0].steps[0]);
        assert!(a.disagreements.is_empty());
        let b = run_pair(&mut c, &methods[0].steps[1]);
        assert_eq!(b.disagreements.len(), 1);
        assert!(b.disagreements[0].why.contains("does not match"));
        assert_eq!(b.category, Some(Category::KernelMismatch));
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
