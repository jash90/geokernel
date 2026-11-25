//! Command forms and the evidence that a form is done.
//!
//! `tools/command-matrix/forms.csv` lists every 2D form of every in-scope
//! command, as an argument signature such as `point,point` (`()` takes no
//! argument, and `...` repeats the token before it). A form is done
//! when `tools/command-matrix/form-evidence.toml` names matching golden cases,
//! a row of `DIFFERENCES.md` that gives it a disposition, or both.
//! `tests/command_forms.rs` checks the rules in `tools/command-matrix/README.md`.

use crate::table::Table;
use geo_kernel::Kind;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Tokens that stand for many kinds. A wildcard form needs a disposition or
/// full case coverage of its supported input kinds.
pub const WILDCARDS: [&str; 3] = ["any", "path", "region"];

/// The columns of `forms.csv`.
pub const FORMS_COLUMNS: [&str; 4] = ["command", "group", "forms_2d", "forms"];

/// The forms of one command.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandForms {
    pub group: String,
    pub forms: Vec<String>,
}

/// Reads `forms.csv`: command name to its forms, checking that the count
/// column agrees with the list and that no form is listed twice.
pub fn parse_forms(text: &str) -> Result<BTreeMap<String, CommandForms>, String> {
    let t = Table::parse(text, "forms.csv")?;
    if t.header != FORMS_COLUMNS {
        return Err(format!("forms.csv: header {:?}", t.header));
    }
    let mut out = BTreeMap::new();
    for r in &t.rows {
        let forms: Vec<String> = if r[3].is_empty() {
            Vec::new()
        } else {
            r[3].split(" | ").map(str::to_string).collect()
        };
        let n: usize = r[2]
            .parse()
            .map_err(|_| format!("forms.csv: `{}`: bad count `{}`", r[0], r[2]))?;
        if n != forms.len() {
            return Err(format!(
                "forms.csv: `{}` says {n} forms and lists {}",
                r[0],
                forms.len()
            ));
        }
        for (i, f) in forms.iter().enumerate() {
            if forms[..i].contains(f) {
                return Err(format!("forms.csv: `{}` lists `{f}` twice", r[0]));
            }
            if f != "()" && f.split(',').any(|t| t.is_empty() || t.contains(['|', ' '])) {
                return Err(format!("forms.csv: `{}`: malformed form `{f}`", r[0]));
            }
        }
        let entry = CommandForms {
            group: r[1].clone(),
            forms,
        };
        if out.insert(r[0].clone(), entry).is_some() {
            return Err(format!("forms.csv: `{}` is listed twice", r[0]));
        }
    }
    Ok(out)
}

/// One row of `form-evidence.toml`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub command: String,
    pub form: String,
    /// Golden case ids that call the command in this form.
    #[serde(default)]
    pub cases: Vec<String>,
    /// A row of `DIFFERENCES.md`, written `section-slug: Area text`.
    pub difference: Option<String>,
    /// The reference refuses the form: the cited cases show it with a
    /// `reject` step, which the replay checks against the kernel.
    #[serde(default)]
    pub refusal: bool,
    /// The form agrees only with the kernel's `giac` feature: its cases are
    /// checked against `known-gaps-giac.toml`, and every recorded case that
    /// calls the form must agree there (docs/cas-giac.md).
    #[serde(default)]
    pub giac: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceFile {
    #[serde(default)]
    form: Vec<Evidence>,
}

/// Reads `form-evidence.toml`: every row has cases, a difference, or both,
/// and no form appears twice.
pub fn parse_evidence(text: &str) -> Result<Vec<Evidence>, String> {
    let file: EvidenceFile =
        toml::from_str(text).map_err(|e| format!("form-evidence.toml: {e}"))?;
    let mut seen = std::collections::BTreeSet::new();
    for e in &file.form {
        let what = format!("form-evidence.toml: `{}` `{}`", e.command, e.form);
        if e.cases.is_empty() && e.difference.is_none() {
            return Err(format!("{what}: needs `cases` or `difference`"));
        }
        if !seen.insert((e.command.clone(), e.form.clone())) {
            return Err(format!("{what} is listed twice"));
        }
    }
    Ok(file.form)
}

/// Whether a form has a wildcard token.
pub fn is_wildcard(form: &str) -> bool {
    form.split(',').any(|t| WILDCARDS.contains(&t))
}

/// The form tokens an argument of kind `k` satisfies. Exact kinds only: a
/// segment is not taken for `line` and a circle not for `conic` (upstream
/// accepts both, but the kernel keeps them as kinds of their own, so a case
/// with a segment or a circle does not show the form for a general line or
/// conic).
fn tokens_of(k: Kind) -> &'static [&'static str] {
    match k {
        Kind::Point => &["point"],
        Kind::Vector => &["vector"],
        Kind::Number => &["number"],
        Kind::Angle => &["angle"],
        Kind::Boolean => &["boolean"],
        Kind::Line => &["line"],
        Kind::Segment => &["segment"],
        Kind::Ray => &["ray"],
        Kind::Circle => &["circle"],
        Kind::Conic => &["conic"],
        Kind::Arc => &["arc"],
        Kind::Sector => &["sector"],
        Kind::Polygon => &["polygon"],
        Kind::PolyLine => &["polyline"],
        Kind::List => &["list"],
        Kind::Function | Kind::FunctionNVar => &["function"],
        Kind::Curve => &["curve"],
        Kind::ImplicitCurve => &["implicit"],
        Kind::Text => &["text"],
        Kind::Image => &["image"],
        Kind::Locus => &["locus"],
        _ => &[],
    }
}

fn token_matches(token: &str, kind: Kind) -> bool {
    match token {
        "any" => true,
        "path" => matches!(
            kind,
            Kind::Arc
                | Kind::Circle
                | Kind::Conic
                | Kind::Curve
                | Kind::Function
                | Kind::Line
                | Kind::List
                | Kind::Locus
                | Kind::Polygon
                | Kind::PolyLine
                | Kind::Ray
                | Kind::Sector
                | Kind::Segment
        ),
        "region" => matches!(
            kind,
            Kind::Arc | Kind::Circle | Kind::Conic | Kind::Polygon | Kind::Sector | Kind::Function
        ),
        _ => tokens_of(kind).contains(&token),
    }
}

/// Whether arguments of the given kinds (`None`: not known) fit `form`.
pub fn form_matches(form: &str, args: &[Option<Kind>]) -> bool {
    let mut toks: Vec<&str> = if form == "()" {
        Vec::new()
    } else {
        form.split(',').collect()
    };
    if let Some(repeat) = toks.iter().position(|&token| token == "...") {
        if repeat == 0 || args.len() < toks.len() - 1 {
            return false;
        }
        let token = toks[repeat - 1];
        let extra = args.len() - (toks.len() - 1);
        toks.splice(repeat..=repeat, std::iter::repeat_n(token, extra));
    }
    toks.len() == args.len()
        && toks
            .iter()
            .zip(args)
            .all(|(token, arg)| arg.is_some_and(|kind| token_matches(token, kind)))
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The characters outside string literals, with their byte positions. A
/// backslash inside a literal escapes the character after it.
fn unquoted(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut quoted = false;
    let mut escaped = false;
    text.char_indices().filter(move |&(_, c)| {
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else {
            return !quoted;
        }
        false
    })
}

/// The index just past the bracket that closes the one at `open`.
fn closing(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in unquoted(&s[open..]) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits at the commas outside brackets and string literals; an empty
/// text has no argument.
pub fn split_args(s: &str) -> Vec<String> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in unquoted(s) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim().to_string());
    out
}

/// Every call `Name(args)` in an expression, nested ones included, in order.
/// What a string literal holds is text, not a call.
pub fn calls(expr: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut prev: Option<(usize, char)> = None;
    for (i, c) in unquoted(expr) {
        // A name starts after something that is no part of a name; the
        // closing quote of a literal sits between two unquoted characters.
        let adjacent = prev.is_some_and(|(at, p)| at + p.len_utf8() == i);
        let starts = c.is_alphabetic()
            && !(adjacent && prev.is_some_and(|(_, p)| is_ident_char(p) || p == '{'));
        prev = Some((i, c));
        if !starts {
            continue;
        }
        let end = expr[i..]
            .find(|ch: char| !is_ident_char(ch))
            .map_or(expr.len(), |n| i + n);
        if expr[end..].starts_with('(') {
            if let Some(close) = closing(expr, end) {
                out.push((
                    expr[i..end].to_string(),
                    split_args(&expr[end + 1..close - 1]),
                ));
            }
        }
    }
    out
}

/// The kind of an argument, where it can be told without the kernel: a
/// label of the case, a number written with digits, operators and number
/// labels, a comparison (a boolean), a list in braces, a pair of numbers (a
/// point). Recognised nested calls with a known output can also supply a
/// kind. Other calls and names outside the case labels are unknown.
pub fn arg_kind(arg: &str, labels: &BTreeMap<String, Kind>) -> Option<Kind> {
    let a = arg.trim();
    if let Some(&k) = labels.get(a) {
        return Some(k);
    }
    // The axes are lines that no snapshot lists.
    if a == "xAxis" || a == "yAxis" {
        return Some(Kind::Line);
    }
    if a.starts_with('"') && a.ends_with('"') {
        return Some(Kind::Text);
    }
    if a == "true" || a == "false" {
        return Some(Kind::Boolean);
    }
    let list_literal =
        |value: &str| value.starts_with('{') && closing(value, 0) == Some(value.len());
    if list_literal(a)
        || a.strip_prefix("Sequence").is_some_and(|args| {
            let args = args.trim_start();
            args.starts_with('(') && closing(args, 0) == Some(args.len())
        })
    {
        return Some(Kind::List);
    }
    // Line has one output kind across its two-argument forms.
    if a.starts_with("Line(") {
        let (_, args) = geo_kernel::script::split_call(a)?;
        if args.len() == 2 {
            return Some(Kind::Line);
        }
    }
    // Dilating a list preserves its kind. This identifies the Length(list)
    // probe in dilation cases without attributing its gap to other overloads.
    if a.starts_with("Dilate(") {
        let (_, args) = geo_kernel::script::split_call(a)?;
        if let [source, _] | [source, _, _] = args.as_slice() {
            if list_literal(source) || labels.get(*source) == Some(&Kind::List) {
                return Some(Kind::List);
            }
        }
    }
    if a.starts_with('(') && closing(a, 0) == Some(a.len()) {
        let inner = split_args(&a[1..a.len() - 1]);
        if inner.len() == 2
            && inner
                .iter()
                .all(|x| arg_kind(x, labels) == Some(Kind::Number))
        {
            return Some(Kind::Point);
        }
    }
    // No call, list or tuple inside: only names, digits and operators.
    let has_call = a
        .char_indices()
        .any(|(i, c)| c == '(' && a[..i].chars().last().is_some_and(is_ident_char));
    if has_call || a.contains([',', '"']) {
        return None;
    }
    // `2a` is two times `a`: digits, then a number label, `pi` or nothing.
    let names_are_numbers = a
        .split(|c: char| !(is_ident_char(c) || "._{}".contains(c)))
        .filter(|w| !w.is_empty())
        .all(|w| {
            let name = w.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
            name.is_empty() || name == "pi" || labels.get(name) == Some(&Kind::Number)
        });
    if !names_are_numbers || !a.chars().any(|c| c.is_ascii_digit() || c.is_alphabetic()) {
        return None;
    }
    const COMPARISONS: [&str; 10] = ["<", ">", "==", "!=", "≟", "≠", "≤", "≥", "&&", "||"];
    if COMPARISONS.iter().any(|op| a.contains(op)) {
        return Some(Kind::Boolean);
    }
    if a.chars()
        .all(|c| is_ident_char(c) || " .+-*/^(){}".contains(c))
    {
        return Some(Kind::Number);
    }
    None
}

/// Whether `lhs` names what a line defines: a label (`a_{1}`, `A'`), a
/// function label with its variables (`f(x)`, `g(x, y)`) or a list of
/// labels in braces (`{A, B}`).
fn is_label_side(lhs: &str) -> bool {
    fn label(s: &str) -> bool {
        let s = s.trim();
        s.chars().next().is_some_and(char::is_alphabetic)
            && s.chars().all(|c| is_ident_char(c) || "{}'".contains(c))
    }
    let lhs = lhs.trim();
    if let Some(list) = lhs.strip_prefix('{').and_then(|l| l.strip_suffix('}')) {
        return !list.trim().is_empty() && list.split(',').all(label);
    }
    match lhs.find('(') {
        Some(open) if lhs.ends_with(')') => {
            label(&lhs[..open]) && lhs[open + 1..lhs.len() - 1].split(',').all(label)
        }
        _ => label(lhs),
    }
}

/// What a script line defines or runs, without its comment: the text after
/// `label =`, `label :` or `label :=` when the text before the first `=` or
/// `:` outside brackets and string literals names a label
/// ([`is_label_side`]), otherwise the whole line, so a standalone command
/// keeps an argument that holds an equation (`Solve(x^2 = 4, x)`).
fn definition(line: &str) -> &str {
    let line = geo_kernel::script::split_comment(line).0;
    let mut depth = 0i32;
    let mut prev: Option<char> = None;
    for (i, c) in unquoted(line) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '=' | ':' if depth == 0 => {
                let rest = &line[i + 1..];
                // a comparison is no definition
                if c == '=' && (rest.starts_with('=') || prev.is_some_and(|p| "<>!".contains(p))) {
                    return line;
                }
                if !is_label_side(&line[..i]) {
                    return line;
                }
                return if c == ':' {
                    rest.strip_prefix('=').unwrap_or(rest)
                } else {
                    rest
                };
            }
            _ => {}
        }
        prev = Some(c);
    }
    line
}

/// The right-hand sides and standalone commands of a script, without comments.
fn definitions(script: &[String]) -> impl Iterator<Item = &str> {
    script.iter().map(|l| definition(l))
}

/// Whether a script calls `command` (through any of its names in
/// `names`, which maps every name to the command it means) with arguments
/// that fit `form`; `labels` gives the kind of every object of the case.
pub fn script_uses_form(
    script: &[String],
    labels: &BTreeMap<String, Kind>,
    names: &BTreeMap<String, String>,
    command: &str,
    form: &str,
) -> bool {
    definitions(script).any(|rhs| {
        calls(rhs).iter().any(|(name, args)| {
            names.get(name).is_some_and(|c| c == command)
                && call_fits(command, form, args, &call_kinds(command, args, labels))
        })
    })
}

/// The kinds of the arguments of a call of `command`, as far as they can be
/// told without the kernel (see [`arg_kind`]), with the readings particular
/// to some commands: a bound variable, an inequality typed in place.
fn call_kinds(
    command: &str,
    args: &[String],
    labels: &BTreeMap<String, Kind>,
) -> Vec<Option<Kind>> {
    let mut kinds: Vec<Option<Kind>> = args.iter().map(|a| arg_kind(a, labels)).collect();
    if command == "Cubic" && matches!(kinds.as_slice(), [_, _, _, Some(Kind::Angle)]) {
        kinds[3] = Some(Kind::Number);
    }
    if command == "TriangleCurve"
        && matches!(args, [_, _, _, equation]
                        if equation.matches('=').count() == 1
                            && !["<=", ">=", "!="].iter().any(|op| equation.contains(op)))
    {
        kinds[3] = Some(Kind::ImplicitCurve);
    }
    if matches!(
        command,
        "IntersectPath" | "IntersectRegion" | "IntersectionPaths"
    ) {
        // The region clipping overload uses circles and conic parts
        // as conic regions.
        for kind in &mut kinds {
            if matches!(kind, Some(Kind::Circle | Kind::Arc | Kind::Sector)) {
                *kind = Some(Kind::Conic);
            }
        }
    }
    if matches!(command, "If" | "CountIf" | "KeepIf") {
        match (args, kinds.as_mut_slice()) {
            ([test, ..], [first, ..]) if inline_inequality(test) => {
                *first = Some(Kind::Function);
            }
            ([test, variable, _], [first, binding, _])
                if matches!(command, "CountIf" | "KeepIf")
                    && variable.chars().all(is_ident_char)
                    && test.contains(['<', '>', '≤', '≥', '=']) =>
            {
                *first = Some(Kind::Boolean);
                // Only the name of the bound local matters for this form.
                *binding = Some(Kind::Number);
            }
            _ => {}
        }
    }
    if command == "BarChart" && matches!(args.len(), 6 | 7) {
        let variable = args[3].trim();
        if variable.chars().next().is_some_and(char::is_alphabetic)
            && variable.chars().all(is_ident_char)
        {
            let mut scope = labels.clone();
            scope.insert(variable.to_owned(), Kind::Number);
            kinds[2] = arg_kind(&args[2], &scope);
            kinds[3] = Some(Kind::Number);
        }
    }
    if matches!(command, "Solve" | "Solutions" | "NSolve" | "NSolutions") && args.len() == 2 {
        // The second argument counts only as a variable of the
        // equation, an assignment to one, or a list of them.
        kinds[1] = solving_variables(&args[0], &args[1], labels).then_some(Kind::Number);
    }
    let binding = match (command, args.len()) {
        ("CurveCartesian", 4) | ("Function", 4) => Some(1),
        ("CurveCartesian", 5) => Some(2),
        _ => None,
    };
    if let Some(i) = binding {
        let mut chars = args[i].chars();
        if chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(is_ident_char) {
            kinds[i] = Some(Kind::Number);
        }
    }
    kinds
}

/// Whether a call with these arguments shows `form` exactly.
fn call_fits(command: &str, form: &str, args: &[String], kinds: &[Option<Kind>]) -> bool {
    if matches!(
        command,
        "CellRange" | "Row" | "Column" | "ColumnName" | "FillCells"
    ) {
        let tokens: Vec<_> = form.split(',').collect();
        let is_cell = |arg: &str| geo_kernel::spreadsheet::Address::parse(arg.trim()).is_some();
        return tokens.len() == args.len()
            && tokens.iter().zip(args.iter().zip(kinds)).all(
                |(&token, (arg, kind))| match token {
                    "cell" => is_cell(arg),
                    "range" => arg.split_once(':').is_some_and(|(a, b)| is_cell(a) && is_cell(b))
                        || geo_kernel::script::split_call(arg).is_some_and(|(name, bounds)| {
                            name == "CellRange"
                                && matches!(bounds.as_slice(), [a, b] | [a, b, _] if is_cell(a) && is_cell(b))
                        }),
                    "type" => true,
                    "object" => is_cell(arg) || kind.is_some(),
                    _ => kind.is_some_and(|kind| token_matches(token, kind)),
                },
            );
    }
    if matches!(form, "inequality" | "interval") {
        return matches!(args, [arg] if inline_inequality(arg));
    }
    if command == "Defined" && form == "any" {
        return args.len() == 1;
    }
    // Length's conic overload measures the arc of a conic part.
    if command == "Length" && form == "conic" && matches!(kinds, [Some(Kind::Arc | Kind::Sector)]) {
        return true;
    }
    form_matches(form, kinds)
}

/// Whether a token takes a kind the way upstream's command processors do:
/// a circle or a conic part is a conic there, a segment or a ray a line and
/// an angle a number.
fn token_may_take(token: &str, kind: Kind) -> bool {
    token_matches(token, kind)
        || match token {
            "conic" => matches!(kind, Kind::Circle | Kind::Arc | Kind::Sector),
            "line" => matches!(kind, Kind::Segment | Kind::Ray),
            "number" => kind == Kind::Angle,
            _ => false,
        }
}

/// The kinds an argument of unknown kind may have, from its shape alone;
/// `None` when it may be anything (it holds a call, a list or a text).
/// Free names with operators make a number or a function, a pair a point
/// or a vector, an equation a line, a conic, an implicit curve or a
/// function, and a comparison a boolean or an inequality.
fn possible_kinds(arg: &str) -> Option<&'static [Kind]> {
    let a = arg.trim();
    let has_call = a
        .char_indices()
        .any(|(i, c)| c == '(' && a[..i].chars().last().is_some_and(is_ident_char));
    if has_call || a.contains(['{', '"']) {
        return None;
    }
    if a.starts_with('(')
        && closing(a, 0) == Some(a.len())
        && split_args(&a[1..a.len() - 1]).len() > 1
    {
        return Some(&[Kind::Point, Kind::Vector]);
    }
    if a.contains(',') {
        return None;
    }
    const COMPARISONS: [&str; 10] = ["<", ">", "==", "!=", "≟", "≠", "≤", "≥", "&&", "||"];
    if COMPARISONS.iter().any(|op| a.contains(op)) {
        return Some(&[
            Kind::Boolean,
            Kind::Function,
            Kind::FunctionNVar,
            Kind::ImplicitCurve,
        ]);
    }
    if a.contains('=') {
        return Some(&[
            Kind::Line,
            Kind::Conic,
            Kind::Circle,
            Kind::ImplicitCurve,
            Kind::Function,
            Kind::FunctionNVar,
        ]);
    }
    Some(&[
        Kind::Number,
        Kind::Angle,
        Kind::Function,
        Kind::FunctionNVar,
    ])
}

/// Whether arguments of these kinds may reach `form` upstream, reading every
/// token as [`token_may_take`] does; an argument of unknown kind may have
/// any of its [`possible_kinds`].
fn form_may_take(form: &str, args: &[String], kinds: &[Option<Kind>]) -> bool {
    let mut toks: Vec<&str> = if form == "()" {
        Vec::new()
    } else {
        form.split(',').collect()
    };
    if let Some(repeat) = toks.iter().position(|&token| token == "...") {
        if repeat == 0 || args.len() < toks.len() - 1 {
            return false;
        }
        let token = toks[repeat - 1];
        let extra = args.len() - (toks.len() - 1);
        toks.splice(repeat..=repeat, std::iter::repeat_n(token, extra));
    }
    toks.len() == args.len()
        && toks
            .iter()
            .zip(args)
            .zip(kinds)
            .all(|((token, arg), kind)| match kind {
                Some(kind) => token_may_take(token, *kind),
                None => possible_kinds(arg)
                    .is_none_or(|kinds| kinds.iter().any(|&k| token_may_take(token, k))),
            })
}

/// Whether a script may call `command` in `form`, one of its forms `all`:
/// a call goes to the forms it shows exactly ([`script_uses_form`]) when
/// there are any, and otherwise to every form its arguments may reach
/// upstream, an argument of unknown kind (a nested call) reaching any token.
/// This errs towards a call reaching a form, never away from it.
pub fn script_may_use_form(
    script: &[String],
    labels: &BTreeMap<String, Kind>,
    names: &BTreeMap<String, String>,
    command: &str,
    all: &[String],
    form: &str,
) -> bool {
    definitions(script).any(|rhs| {
        calls(rhs).iter().any(|(name, args)| {
            if names.get(name).is_none_or(|c| c != command) {
                return false;
            }
            let kinds = call_kinds(command, args, labels);
            let exact: Vec<&String> = all
                .iter()
                .filter(|f| call_fits(command, f, args, &kinds))
                .collect();
            if !exact.is_empty() {
                return exact.iter().any(|f| *f == form);
            }
            if matches!(form, "inequality" | "interval") {
                return false;
            }
            form_may_take(form, args, &kinds)
        })
    })
}

/// Whether `hint` names variables of the equation `equation` of a solve:
/// one variable, `variable = value`, or a list of variables. A function
/// holds `x`; a line, a conic, an implicit curve or a list of them `x` and
/// `y`; an equation typed in place the free names it uses and the
/// variables of the objects it names.
fn solving_variables(equation: &str, hint: &str, labels: &BTreeMap<String, Kind>) -> bool {
    let equation = equation.trim();
    let variables: BTreeSet<String> = match labels.get(equation) {
        Some(Kind::Function) => ["x".to_owned()].into(),
        Some(Kind::Line | Kind::Conic | Kind::Circle | Kind::ImplicitCurve | Kind::List) => {
            ["x".to_owned(), "y".to_owned()].into()
        }
        Some(_) => return false,
        None => {
            let mut names = BTreeSet::new();
            let mut rest = equation;
            while let Some(start) = rest.find(|c: char| c.is_alphabetic()) {
                let tail = &rest[start..];
                let end = tail.find(|c: char| !is_ident_char(c)).unwrap_or(tail.len());
                let name = &tail[..end];
                let call = tail[end..].trim_start().starts_with('(');
                match labels.get(name) {
                    _ if call => {}
                    None => {
                        names.insert(name.to_owned());
                    }
                    Some(Kind::Function) => {
                        names.insert("x".to_owned());
                    }
                    Some(Kind::Line | Kind::Conic | Kind::Circle | Kind::ImplicitCurve) => {
                        names.extend(["x".to_owned(), "y".to_owned()]);
                    }
                    Some(_) => {}
                }
                rest = &tail[end..];
            }
            names
        }
    };
    let hint = hint.trim();
    let (listed, items) = match hint.strip_prefix('{').and_then(|h| h.strip_suffix('}')) {
        Some(inner) => (true, split_args(inner)),
        None => (false, vec![hint.to_owned()]),
    };
    !items.is_empty()
        && items.iter().all(|item| {
            let (name, value) = match item.split_once('=') {
                Some((name, value)) if !listed => (name.trim(), Some(value)),
                Some(_) => return false,
                None => (item.trim(), None),
            };
            variables.contains(name) && value.is_none_or(|v| !v.trim().is_empty())
        })
}

/// An inline one-variable inequality, as opposed to a scalar comparison
/// or the coordinate extractor `x(P)`.
fn inline_inequality(arg: &str) -> bool {
    arg.contains(['<', '>', '≤', '≥'])
        && arg.match_indices('x').any(|(i, _)| {
            !arg[..i].ends_with(is_ident_char)
                && !arg[i + 1..].starts_with(is_ident_char)
                && !arg[i + 1..].trim_start().starts_with('(')
        })
}

/// Whether a script calls `command` at all.
pub fn script_calls(script: &[String], names: &BTreeMap<String, String>, command: &str) -> bool {
    definitions(script).any(|rhs| {
        calls(rhs)
            .iter()
            .any(|(name, _)| names.get(name).is_some_and(|c| c == command))
    })
}

/// The slug a Markdown heading gets as a link target: lower case, letters,
/// digits, `-` and `_` kept, spaces turned into `-`.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// One table row of `DIFFERENCES.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct DifferenceRow {
    /// Slug of the heading the row is under.
    pub section: String,
    /// The first cell.
    pub area: String,
    /// The last cell.
    pub reason: String,
}

/// The cells of a Markdown table row; `\|` does not split.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = vec![String::new()];
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            out.last_mut().unwrap().push('|');
            chars.next();
        } else if c == '|' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out.iter().map(|c| c.trim().to_string()).collect()
}

/// The table rows of `DIFFERENCES.md` with their section, header and
/// separator rows left out.
pub fn difference_rows(md: &str) -> Vec<DifferenceRow> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in md.lines() {
        if let Some(h) = line.strip_prefix('#') {
            section = slug(h.trim_start_matches('#'));
            continue;
        }
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let c = cells(line);
        let separator = c.iter().all(|x| x.chars().all(|ch| ch == '-' || ch == ':'));
        if separator || c[0] == "Area" || c.len() < 2 {
            continue;
        }
        out.push(DifferenceRow {
            section: section.clone(),
            area: c[0].clone(),
            reason: c.last().unwrap().clone(),
        });
    }
    out
}

/// How a row of `DIFFERENCES.md` names a form it disposes of: the command
/// as the matrix names it with the form's tokens as arguments, in backticks,
/// such as `` `Mod(function, function)` `` or `` `AxisStepX()` ``.
pub fn form_key(command: &str, form: &str) -> String {
    let args = if form == "()" {
        String::new()
    } else {
        form.split(',').collect::<Vec<_>>().join(", ")
    };
    format!("`{command}({args})`")
}

/// Checks a `difference` reference of the form `form` of `command` against
/// the rows of `DIFFERENCES.md`: the row must exist, its first cell must
/// name the form with [`form_key`], and its reason must be a disposition,
/// not "Not implemented".
pub fn check_difference(
    reference: &str,
    command: &str,
    form: &str,
    rows: &[DifferenceRow],
) -> Result<(), String> {
    let Some((section, area)) = reference.split_once(": ") else {
        return Err(format!("`{reference}` is not `section-slug: Area text`"));
    };
    let Some(row) = rows.iter().find(|r| r.section == section && r.area == area) else {
        return Err(format!("no row `{area}` under the heading `{section}`"));
    };
    let key = form_key(command, form);
    if !row.area.contains(&key) {
        return Err(format!(
            "the row `{area}` does not name the form; its first cell must contain {key}"
        ));
    }
    if row.reason.starts_with("Not implemented") {
        return Err(format!(
            "the row `{area}` only says the form is not implemented; that is no disposition"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(pairs: &[(&str, Kind)]) -> BTreeMap<String, Kind> {
        pairs.iter().map(|(l, k)| (l.to_string(), *k)).collect()
    }

    #[test]
    fn forms_csv_counts_must_agree() {
        let ok = "command,group,forms_2d,forms\nMod,algebra,2,\"number,number | function,function\"\nType,geogebra,0,\nAxisStepX,geogebra,1,()\n";
        let f = parse_forms(ok).unwrap();
        assert_eq!(f["Mod"].forms, ["number,number", "function,function"]);
        assert!(f["Type"].forms.is_empty());
        assert_eq!(f["AxisStepX"].forms, ["()"]);
        assert!(
            parse_forms("command,group,forms_2d,forms\nMod,algebra,3,\"number,number\"\n").is_err()
        );
        assert!(parse_forms("command,group,forms_2d,forms\nM,a,2,\"number | number\"\n").is_err());
        assert!(parse_forms("command,group,forms_2d,forms\nM,a,1,\"number|list\"\n").is_err());
    }

    #[test]
    fn evidence_needs_cases_or_a_difference() {
        let one = "[[form]]\ncommand = \"Mod\"\nform = \"number,number\"\ncases = [\"x-001\"]\n";
        assert_eq!(parse_evidence(one).unwrap()[0].cases, ["x-001"]);
        assert!(parse_evidence("[[form]]\ncommand = \"Mod\"\nform = \"number,number\"\n").is_err());
        assert!(parse_evidence(&format!("{one}{one}")).is_err());
        let both =
            "[[form]]\ncommand = \"M\"\nform = \"x\"\ncases = [\"a\"]\ndifference = \"s: a\"\n";
        let evidence = parse_evidence(both).unwrap();
        assert_eq!(evidence[0].cases, ["a"]);
        assert_eq!(evidence[0].difference.as_deref(), Some("s: a"));
        assert!(parse_evidence("[[form]]\ncommand = \"M\"\nform = \"x\"\nwhy = 1\n").is_err());
    }

    #[test]
    fn forms_match_exact_kinds_and_repeats() {
        let p = Some(Kind::Point);
        let n = Some(Kind::Number);
        assert!(form_matches("point,point", &[p, p]));
        assert!(!form_matches("point,point", &[p, n]));
        assert!(!form_matches("point,point", &[p]));
        assert!(form_matches("()", &[]));
        assert!(form_matches("point,point,...", &[p, p, p, p]));
        assert!(!form_matches("point,point,...", &[p]));
        assert!(form_matches("point,number,...", &[p, n, n]));
        let b = Some(Kind::Boolean);
        assert!(form_matches("point,point,...,boolean", &[p, p, p, b]));
        assert!(!form_matches("point,point,...,boolean", &[p, p, b, p]));
        assert!(form_matches("any,any", &[p, b]));
        assert!(form_matches("path,number", &[Some(Kind::Arc), n]));
        assert!(form_matches("region", &[Some(Kind::Polygon)]));
        assert!(!form_matches("region", &[Some(Kind::Line)]));
        assert!(!form_matches("conic", &[Some(Kind::Circle)]));
        assert!(!form_matches("line", &[Some(Kind::Segment)]));
        assert!(!form_matches("number", &[None]));
        assert!(is_wildcard("any,vector") && is_wildcard("point,path") && !is_wildcard("point"));
    }

    #[test]
    fn calls_are_found_nested_and_split_at_top_level_commas() {
        assert_eq!(
            calls("Midpoint(Segment(A, B)) + f(2, {1, 2})"),
            [
                ("Midpoint".to_string(), vec!["Segment(A, B)".to_string()]),
                ("Segment".to_string(), vec!["A".into(), "B".into()]),
                ("f".to_string(), vec!["2".into(), "{1, 2}".into()]),
            ]
        );
        assert_eq!(calls("AxisStepX()"), [("AxisStepX".to_string(), vec![])]);
        // `a_{1}(` is no call of `1`, and a name inside a word is no call
        assert_eq!(calls("ab(1)")[0].0, "ab");
        assert_eq!(calls("2 a").len(), 0);
    }

    #[test]
    fn a_call_inside_a_text_is_no_call() {
        assert_eq!(calls("\"Solve(f, x)\"").len(), 0);
        assert_eq!(calls("\"a \\\" Solve(f, x)\" + \"Factor(f)\"").len(), 0);
        assert_eq!(
            calls("Simplify(\"a, b = Expand(c)\")"),
            [(
                "Simplify".to_string(),
                vec!["\"a, b = Expand(c)\"".to_string()]
            )]
        );
        // The bracket inside the text closes nothing.
        assert_eq!(
            calls("Length(\"(a\") + Factor(f)"),
            [
                ("Length".to_string(), vec!["\"(a\"".to_string()]),
                ("Factor".to_string(), vec!["f".to_string()]),
            ]
        );
        // A name right after a text is a name of its own.
        assert_eq!(calls("\"a\"Factor(f)")[0].0, "Factor");
        assert_eq!(split_args("\"a,b\", c"), ["\"a,b\"", "c"]);
    }

    #[test]
    fn the_solving_variable_is_no_existing_object() {
        let l = labels(&[
            ("f", Kind::Function),
            ("P", Kind::Point),
            ("b", Kind::Number),
            ("s", Kind::Text),
        ]);
        let names: BTreeMap<String, String> = [("Solve", "Solve")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let uses = |line: &str, form: &str| {
            script_uses_form(&[line.to_string()], &l, &names, "Solve", form)
        };
        assert!(uses("r = Solve(f, x)", "function,number"));
        assert!(uses("r = Solve(f, x=1)", "function,any"));
        assert!(uses("r = Solve(f, {x})", "function,any"));
        // Not a variable of the equation: a typo, a number, a point, a text.
        assert!(!uses("r = Solve(f, typo)", "function,any"));
        assert!(!uses("r = Solve(f, b)", "function,any"));
        assert!(!uses("r = Solve(f, P)", "function,any"));
        assert!(!uses("r = Solve(f, P)", "function,point"));
        assert!(!uses("r = Solve(f, s)", "function,any"));
        assert!(!uses("r = Solve(f, {x, typo})", "function,any"));
        // A call quoted in a text is no evidence.
        assert!(!uses("r = \"Solve(f, x)\"", "function,number"));
    }

    #[test]
    fn argument_kinds() {
        let l = labels(&[
            ("A", Kind::Point),
            ("a", Kind::Number),
            ("s", Kind::Segment),
        ]);
        assert_eq!(arg_kind("A", &l), Some(Kind::Point));
        assert_eq!(arg_kind(" s ", &l), Some(Kind::Segment));
        assert_eq!(arg_kind("-2.5", &l), Some(Kind::Number));
        assert_eq!(arg_kind("2a + 1", &l), Some(Kind::Number));
        assert_eq!(arg_kind("a^2 - pi", &l), Some(Kind::Number));
        assert_eq!(arg_kind("k", &l), None);
        assert_eq!(arg_kind("xAxis", &l), Some(Kind::Line));
        assert_eq!(arg_kind(" yAxis ", &l), Some(Kind::Line));
        assert_eq!(arg_kind("a < 3", &l), Some(Kind::Boolean));
        assert_eq!(arg_kind("true", &l), Some(Kind::Boolean));
        assert_eq!(arg_kind("(1, a)", &l), Some(Kind::Point));
        assert_eq!(arg_kind("{1, 2}", &l), Some(Kind::List));
        assert_eq!(arg_kind("Sequence(i*a,i,1,200000)", &l), Some(Kind::List));
        assert_eq!(arg_kind("Sequence(5) + a", &l), None);
        assert_eq!(arg_kind("Dilate({A}, 2)", &l), Some(Kind::List));
        assert_eq!(arg_kind("Dilate(A, 2)", &l), None);
        assert_eq!(arg_kind("A + (1, 0)", &l), None);
        assert_eq!(arg_kind("sqrt(a)", &l), None);
        assert_eq!(arg_kind("Segment(A, A)", &l), None);
        assert_eq!(arg_kind("Line(A, A)", &l), Some(Kind::Line));
        assert_eq!(arg_kind("s + 1", &l), None);
        assert_eq!(arg_kind("(A, 1)", &l), None);
        assert_eq!(arg_kind("x < 0", &l), None);
        assert_eq!(arg_kind("x^2", &l), None);
        let l2 = labels(&[("a_{1}", Kind::Number)]);
        assert_eq!(arg_kind("a_{1} + 1", &l2), Some(Kind::Number));
    }

    #[test]
    fn scripts_use_a_form_through_any_name() {
        let l = labels(&[("A", Kind::Point), ("B", Kind::Point), ("k", Kind::Circle)]);
        let names: BTreeMap<String, String> =
            [("CircularArc", "CircleArc"), ("CircleArc", "CircleArc")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect();
        let script = vec![
            "A = (0, 0)".into(),
            "c = CircularArc(A, B, A) # note".into(),
        ];
        assert!(script_uses_form(
            &script,
            &l,
            &names,
            "CircleArc",
            "point,point,point"
        ));
        assert!(!script_uses_form(
            &script,
            &l,
            &names,
            "CircleArc",
            "point,point"
        ));
        assert!(script_calls(&script, &names, "CircleArc"));
        assert!(!script_calls(&script, &names, "Circle"));
        // the label of a line is not a call
        let s2 = vec!["Circle = (1, 2)".to_string()];
        assert!(!script_calls(&s2, &names, "CircleArc"));
    }

    #[test]
    fn difference_references_need_a_row_about_the_form_with_a_disposition() {
        let md = "# Deliberate differences\n\n| Area | GeoGebra | geokernel | Reason |\n| --- | --- | --- | --- |\n| `Mod(function, function)` of a \\| bar | x | y | Kept on purpose. |\n| `Div(number, number)` near 0 | x | y | Kept on purpose. |\n\n## General conics (`plot`)\n\n| Area | GeoGebra | geokernel | Reason |\n| --- | --- | --- | --- |\n| `Conic(list)` | a | Refused. | Not implemented. |\n";
        let rows = difference_rows(md);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].section, "deliberate-differences");
        assert_eq!(rows[0].area, "`Mod(function, function)` of a | bar");
        assert_eq!(rows[2].section, "general-conics-plot");
        assert_eq!(form_key("AxisStepX", "()"), "`AxisStepX()`");
        let mod_row = "deliberate-differences: `Mod(function, function)` of a | bar";
        assert!(check_difference(mod_row, "Mod", "function,function", &rows).is_ok());
        // a row about another command or another form of the same command
        assert!(check_difference(mod_row, "Div", "function,function", &rows).is_err());
        assert!(check_difference(mod_row, "Mod", "number,number", &rows).is_err());
        let div_row = "deliberate-differences: `Div(number, number)` near 0";
        assert!(check_difference(div_row, "Mod", "function,function", &rows).is_err());
        // a row that only says "Not implemented"
        assert!(
            check_difference("general-conics-plot: `Conic(list)`", "Conic", "list", &rows).is_err()
        );
        assert!(check_difference("general-conics-plot: Nothing", "Conic", "list", &rows).is_err());
        assert!(check_difference("no separator", "Conic", "list", &rows).is_err());
    }

    #[test]
    fn a_definition_is_split_only_after_a_label() {
        assert_eq!(definition("a = Mod(7, 3)"), " Mod(7, 3)");
        assert_eq!(definition("f(x) = x^2"), " x^2");
        assert_eq!(definition("{A, B} = Intersect(c, d)"), " Intersect(c, d)");
        assert_eq!(definition("a_{1} = 2"), " 2");
        assert_eq!(definition("c: x^2 + y^2 = 4"), " x^2 + y^2 = 4");
        assert_eq!(definition("b := 3"), " 3");
        // a standalone command keeps the equation of its argument
        assert_eq!(definition("Solve(x^2 = 4, x)"), "Solve(x^2 = 4, x)");
        assert_eq!(definition("Tangent(A, y = x^2)"), "Tangent(A, y = x^2)");
        assert_eq!(definition("Text(\"a = b\")"), "Text(\"a = b\")");
        assert_eq!(definition("x^2 = 4"), "x^2 = 4");
        assert_eq!(definition("a == b"), "a == b");
        assert_eq!(definition("r = 1 # r = 2"), " 1 ");
        let names: BTreeMap<String, String> = [("Solve".to_string(), "Solve".to_string())].into();
        let line = ["Solve(x^2 = 4, x)".to_string()];
        assert!(script_calls(&line, &names, "Solve"));
        let tangent: BTreeMap<String, String> =
            [("Tangent".to_string(), "Tangent".to_string())].into();
        let line = ["Tangent(A, y = x^2)".to_string()];
        assert!(script_calls(&line, &tangent, "Tangent"));
        assert!(script_may_use_form(
            &line,
            &[("A".to_string(), Kind::Point)].into(),
            &tangent,
            "Tangent",
            &["point,conic".to_string(), "point,function".to_string()],
            "point,conic",
        ));
    }
}
