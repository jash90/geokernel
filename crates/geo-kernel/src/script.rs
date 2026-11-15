// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/LabelManager.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Construction.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/AlgebraProcessor.java
//   source/shared/common/src/main/java/org/geogebra/common/util/StringUtil.java
//   source/shared/editor-base/src/main/java/org/geogebra/editor/share/input/Character.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! A small input-bar style language, one definition per line:
//!
//! ```text
//! A = (0, 0)              # free point
//! v = (1, 2)              # free vector (the label starts with a lower-case letter)
//! r = 3                   # free number
//! l = Line(A, B)
//! m = Line(P, l)          # GeoGebra form: parallel to l (a line or segment) through P
//! n = PerpendicularLine(P, l)  # GeoGebra name for `Perpendicular`
//! k = Circle(A, B)        # centre A, through B
//! m = Circle(A, r)        # centre A, radius r
//! P = Intersect(l, k, 2)  # second intersection (1 is the default)
//! t = Tangent(P, k)       # two tangents t_{1} and t_{2} when both are defined at creation
//! c = a + 2*b             # arithmetic (see `crate::expr`) where no form above applies
//! f(x) = x^2 - a          # a function of x
//! {t_{1}, t_{2}} = Tangent(P, k)   # the same, with the output labels stated
//! {, g} = AngularBisector(l, m)    # only the second output is created, as `g`
//! t_{1} = (1, 2)         # a label may have the indexed form `prefix_{letters, digits}`
//! a = $point(1, 2)        # a free point whose label starts with a lower-case letter
//! B = $vector(1, 2)       # a free vector whose label starts with an upper-case letter
//! ```
//!
//! `{label, label, ...} = Command(...)` names every output of a command that
//! creates several objects, in GeoGebra's output order; an empty slot is an
//! output that is not created. Which outputs are created follows the labels,
//! not which outputs are defined when the line runs. It is what a saved
//! construction needs, because GeoGebra writes the label of every output it
//! created. One limit: when both tangents from a point are the same line at
//! creation (the point is on the circle) the kernel makes only the first, so a
//! label for the second one is an error, and a missing or empty slot after the
//! last output the kernel makes is accepted.
//!
//! `$vector(x, y)` is GeoGebra's own parser form and gives a free vector for
//! any label. `$point(x, y)` gives a free point for any label; GeoGebra's
//! parser also has a `$point` but it follows the label rule, so this is a
//! deliberate difference (see `DIFFERENCES.md`).

use crate::algo::arc::ArcDef;
use crate::algo::conic::ConicDef;
use crate::algo::conic_parts::ConicPartDef;
use crate::algo::general_conic::GeneralConicDef;
use crate::algo::lines::LineDef;
use crate::algo::measure::MeasureDef;
use crate::algo::point::PointDef;
use crate::algo::predicate::PredicateDef;
use crate::algo::transform::TransformDef;
use crate::algo::vector::VectorDef;
use crate::algo::Group;
use crate::kernel::{Construction, Def, Id, Kind, Value};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

/// Builds a construction from a whole script.
pub fn load(src: &str) -> Result<Construction, ScriptError> {
    let mut c = Construction::new();
    for (i, raw) in src.lines().enumerate() {
        exec(&mut c, raw).map_err(|message| ScriptError {
            line: i + 1,
            message,
        })?;
    }
    Ok(c)
}

/// Runs one line. Blank lines and comments return `Ok(None)`.
pub fn exec(c: &mut Construction, raw: &str) -> Result<Option<Id>, String> {
    let line = raw.split('#').next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(None);
    }
    if line.starts_with('{') {
        return exec_labelled(c, line);
    }
    let (name, rhs) = line.split_once('=').ok_or("expected `name = definition`")?;
    let name = name.trim();
    // `f(x) = ...`: a function of `x`
    if let Some(f) = name.strip_suffix("(x)").map(str::trim_end) {
        if !is_identifier(f) {
            return Err(format!("`{f}` is not a valid object name"));
        }
        check_label(f)?;
        return crate::expr::define_function(c, f, rhs.trim()).map(Some);
    }
    if !is_output_label(name) {
        return Err(format!("`{name}` is not a valid object name"));
    }
    check_label(name)?;
    let rhs = rhs.trim();
    if let Some(def) = parse_forced(rhs) {
        return c.add(name, def?).map(Some).map_err(|e| e.to_string());
    }
    if let Some(result) = crate::algo::polygon::exec(c, name, rhs) {
        return result;
    }
    if let Some(result) = crate::algo::general_conic::exec(c, name, rhs) {
        return result;
    }
    match define(c, name, rhs) {
        Ok(id) => {
            if matches!(c.def(id), Def::Number(_)) {
                // a decimal literal keeps its exact decimal
                crate::expr::record_literal(c, id, rhs);
            }
            Ok(Some(id))
        }
        // A call of a command keeps the command's own error.
        Err(e) if is_single_call(c, rhs) => Err(e),
        Err(_) => crate::expr::define(c, name, rhs, is_vector_label(name)).map(Some),
    }
}

/// The forms of the module documentation other than arithmetic.
fn define(c: &mut Construction, name: &str, rhs: &str) -> Result<Id, String> {
    if let Some(defs) = parse_multi(c, rhs) {
        return add_outputs(c, name, defs?);
    }
    let def = match parse_def(c, rhs)? {
        Def::FreePoint { x, y } if is_vector_label(name) => Def::Vector(VectorDef::Free { x, y }),
        def => def,
    };
    c.add(name, def).map_err(|e| e.to_string())
}

/// Whether `rhs` is exactly one call `Name(...)` of a command: the bracket
/// after the name closes at the end, and the name is neither a built-in
/// function nor an object.
fn is_single_call(c: &Construction, rhs: &str) -> bool {
    let Some(open) = rhs.find('(') else {
        return false;
    };
    let name = rhs[..open].trim();
    // `sin²(a)` is a power of a function, not a call of `sin²`
    if !is_identifier(name)
        || !name.chars().all(crate::expr::is_name_char)
        || crate::expr::is_function_name(name)
        || crate::expr::is_expression_command(name)
        || c.id(name).is_some()
    {
        return false;
    }
    let mut depth = 0usize;
    for (i, ch) in rhs.char_indices().skip_while(|&(i, _)| i < open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1 == rhs.len();
                }
            }
            _ => {}
        }
    }
    false
}

/// `Name(args)` split into the name and the trimmed arguments; `None` for
/// anything else (a point literal, a number, a malformed line).
pub(crate) fn split_call(rhs: &str) -> Option<(&str, Vec<&str>)> {
    if rhs.starts_with('(') {
        return None;
    }
    let open = rhs.find('(')?;
    let inner = rhs[open + 1..].strip_suffix(')')?;
    Some((
        rhs[..open].trim(),
        inner.split(',').map(str::trim).collect(),
    ))
}

/// Asks the groups for a command that creates several objects.
fn parse_multi(c: &Construction, rhs: &str) -> Option<Result<Vec<Def>, String>> {
    fn wrap<G: Group>(r: Result<Vec<G>, String>, f: fn(G) -> Def) -> Result<Vec<Def>, String> {
        r.map(|defs| defs.into_iter().map(f).collect())
    }
    let (func, args) = split_call(rhs)?;
    // asked first: `Tangent` of a conic is not a circle tangent
    if let Some(r) = ConicPartDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::ConicPart));
    }
    if let Some(r) = LineDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Lines));
    }
    if let Some(r) = ConicDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Conic));
    }
    if let Some(r) = MeasureDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Measure));
    }
    if let Some(r) = TransformDef::parse_multi(c, func, &args) {
        return Some(wrap(r, Def::Transform));
    }
    None
}

/// Adds the outputs of one command `name = Command(...)`. The labels follow
/// what GeoGebra's `LabelManager.setLabels` and `Construction.getIndexLabel`
/// give at creation. The kernel's rule:
///
/// - two or more outputs defined at creation: every defined output is
///   created and gets the next free indexed label (`t_{1}`, `t_{2}`, ...);
///   `Vertex` of a hyperbola labels its two defined vertices only;
/// - exactly one defined: only that output is created, labelled `name`;
/// - none defined: only the first output is created, labelled `name`.
///
/// The set of created objects is fixed at creation: an output that was not
/// created is not added later, even when it becomes defined.
/// Returns the id of the first object created.
fn add_outputs(c: &mut Construction, name: &str, defs: Vec<Def>) -> Result<Id, String> {
    if c.id(name).is_some() {
        return Err(format!("object `{name}` already exists"));
    }
    let mut defined = Vec::with_capacity(defs.len());
    for def in &defs {
        defined.push(probe(c, def)?);
    }
    if let [first @ Def::ConicPart(ConicPartDef::Directrix { .. }), second] = defs[..] {
        // `AlgoDirectrix` gives both directrices the same label: with both
        // defined the first keeps `name` and the second gets the next free
        // indexed label (observed with the harness).
        return match (defined[0], defined[1]) {
            (true, true) => {
                let id = c.add(name, first).map_err(|e| e.to_string())?;
                let prefix = name
                    .split('_')
                    .next()
                    .filter(|p| !p.is_empty())
                    .unwrap_or("a");
                let label = index_label(c, prefix);
                c.add(&label, second).map_err(|e| e.to_string())?;
                Ok(id)
            }
            (false, true) => c.add(name, second).map_err(|e| e.to_string()),
            _ => c.add(name, first).map_err(|e| e.to_string()),
        };
    }
    let count = defined.iter().filter(|&&d| d).count();
    if count < 2 {
        let k = defined.iter().position(|&d| d).unwrap_or(0);
        return c.add(name, defs[k]).map_err(|e| e.to_string());
    }
    // Construction.getIndexLabel: the prefix is the name up to the first `_`.
    let prefix = match name.split('_').next() {
        Some(p) if !p.is_empty() => p,
        _ => "a",
    };
    let mut first = None;
    for (def, _) in defs.into_iter().zip(defined).filter(|&(_, d)| d) {
        let label = index_label(c, prefix);
        let id = c.add(&label, def).map_err(|e| e.to_string())?;
        first.get_or_insert(id);
    }
    first.ok_or_else(|| "the command creates no object".into())
}

/// `Construction.buildIndexedLabel`: the first `p_{n}` that is free, where
/// for `n < 10` the short form `p_n` must be free too.
pub(crate) fn index_label(c: &Construction, prefix: &str) -> String {
    // Indexes below the hint are known to be taken (see `label_hint`).
    let family = format!("I:{prefix}");
    let mut n = c.label_hint(&family).max(1);
    loop {
        let long = format!("{prefix}_{{{n}}}");
        if c.id(&long).is_none() && (n >= 10 || c.id(&format!("{prefix}_{n}")).is_none()) {
            c.set_label_hint(&family, n);
            return long;
        }
        n += 1;
    }
}

/// Checks a definition of a group and tells whether it is defined now.
fn probe(c: &Construction, def: &Def) -> Result<bool, String> {
    fn run<G: Group>(c: &Construction, d: &G) -> Result<bool, String> {
        d.check(c).map_err(|e| e.to_string())?;
        Ok(d.compute(c).is_some_and(|v| is_finite(&v)))
    }
    match def {
        Def::Lines(d) => run(c, d),
        Def::Conic(d) => run(c, d),
        Def::Measure(d) => run(c, d),
        Def::Transform(d) => run(c, d),
        Def::ConicPart(d) => run(c, d),
        Def::Path(d) => run(c, d),
        Def::Arc(d) => run(c, d),
        _ => Ok(true),
    }
}

fn is_finite(v: &Value) -> bool {
    match *v {
        Value::Point { x, y } => x.is_finite() && y.is_finite(),
        Value::Line { a, b, c } => a.is_finite() && b.is_finite() && c.is_finite(),
        Value::Segment { x1, y1, x2, y2 } => {
            x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()
        }
        Value::Circle { cx, cy, r } => cx.is_finite() && cy.is_finite() && r.is_finite(),
        Value::Number(v) => v.is_finite(),
        Value::Vector { x, y } => x.is_finite() && y.is_finite(),
        Value::Ray { x, y, a, b, c } => [x, y, a, b, c].iter().all(|v| v.is_finite()),
        Value::Bool(_) => true,
        Value::Function | Value::List { .. } => true,
        Value::Polygon { area } => area.is_finite(),
        Value::PolyLine { length } => length.is_finite(),
        Value::Conic { .. } => true,
        Value::Arc { length } => length.is_finite(),
        Value::Sector { area } => area.is_finite(),
        Value::Undefined => false,
    }
}

/// GeoGebra's `AlgebraProcessor.isVectorLabel`: coordinates `(x, y)` make a
/// vector when the first UTF-16 code unit of the label is lower case by
/// `StringUtil.isLowerCase`, a point otherwise. A label that starts outside
/// the Basic Multilingual Plane starts with a surrogate, so it is a point.
pub fn is_vector_label(name: &str) -> bool {
    name.encode_utf16().next().is_some_and(is_lower_case)
}

/// `StringUtil.isLowerCase`: a letter by [`is_letter`] that Java's
/// `Character.toLowerCase(char)` leaves unchanged.
fn is_lower_case(unit: u16) -> bool {
    let Some(ch) = char::from_u32(u32::from(unit)) else {
        return false;
    };
    let mut lower = ch.to_lowercase();
    is_letter(unit) && lower.next() == Some(ch) && lower.next().is_none()
}

/// `Character.isLetter` of GeoGebra's editor (`org.geogebra.editor.share.input`),
/// a fixed list of ranges of UTF-16 code units.
fn is_letter(c: u16) -> bool {
    matches!(c,
        0x0041..=0x005a // upper case (A-Z)
        | 0x0061..=0x007a // lower case (a-z)
        | 0x00b7 // middle dot (for Catalan)
        | 0x00c0..=0x00d6 // accentuated letters
        | 0x00d8..=0x00f6
        | 0x00f8..=0x01bf
        | 0x01c4..=0x02a8
        | 0x038e..=0x03f5 // Greek
        | 0x0401..=0x0481 // Cyrillic
        | 0x0490..=0x04f9
        | 0x0531..=0x1ffc // Arabic, accentuated and others
        | 0x212f // Euler number
        | 0x3041..=0x3357 // Asian letters
        | 0x4e00..=0xd7a3
        | 0xf71d..=0xfa2d
        | 0xfb13..=0xfdfb // Armenian, Hebrew, Arabic
        | 0xfe80..=0xfefc // Arabic
        | 0xff66..=0xff9d // Katakana
        | 0xffa1..=0xffdc)
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(ch) if ch.is_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '\'')
}

pub(crate) fn parse_def(c: &Construction, rhs: &str) -> Result<Def, String> {
    // (x, y)
    if let Some(inner) = rhs.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return Err("a point needs two coordinates: (x, y)".into());
        }
        return Ok(Def::FreePoint {
            x: parse_number(parts[0])?,
            y: parse_number(parts[1])?,
        });
    }

    // 3.5
    if let Some(v) = crate::expr::plain_number(rhs) {
        return Ok(Def::Number(v));
    }

    // Name(arg, arg, ...)
    let open = rhs
        .find('(')
        .ok_or_else(|| format!("cannot understand `{rhs}`"))?;
    let inner = rhs[open + 1..]
        .strip_suffix(')')
        .ok_or("missing closing `)`")?;
    let func = rhs[..open].trim();
    let args: Vec<&str> = inner.split(',').map(str::trim).collect();
    let obj = |s: &str| object(c, s);

    // Groups are asked first, so that a group can add a new form of a
    // built-in command (for example `Circle` with three points). General
    // conics come first: they claim the transformations of conics.
    if let Some(def) = GeneralConicDef::parse(c, func, &args) {
        return def.map(Def::GeneralConic);
    }
    if let Some(def) = ArcDef::parse(c, func, &args) {
        return def.map(Def::Arc);
    }
    if let Some(def) = ConicPartDef::parse(c, func, &args) {
        return def.map(Def::ConicPart);
    }
    if let Some(def) = VectorDef::parse(c, func, &args) {
        return def.map(Def::Vector);
    }
    if let Some(def) = LineDef::parse(c, func, &args) {
        return def.map(Def::Lines);
    }
    if let Some(def) = ConicDef::parse(c, func, &args) {
        return def.map(Def::Conic);
    }
    if let Some(def) = MeasureDef::parse(c, func, &args) {
        return def.map(Def::Measure);
    }
    if let Some(def) = TransformDef::parse(c, func, &args) {
        return def.map(Def::Transform);
    }
    if let Some(def) = PredicateDef::parse(c, func, &args) {
        return def.map(Def::Predicate);
    }
    if let Some(def) = PointDef::parse(c, func, &args) {
        return def.map(Def::Point);
    }
    if let Some(def) = crate::algo::path::PathDef::parse(c, func, &args) {
        return def.map(Def::Path);
    }

    match (func, args.len()) {
        ("Line", 2) => {
            let first = obj(args[0])?;
            let second = obj(args[1])?;
            // GeoGebra: `Line(P, l)` is the parallel to `l` through `P`.
            Ok(if matches!(c.kind(second), Kind::Line | Kind::Segment) {
                Def::Parallel {
                    through: first,
                    to: second,
                }
            } else {
                Def::Line(first, second)
            })
        }
        ("Segment", 2) => Ok(Def::Segment(obj(args[0])?, obj(args[1])?)),
        ("Midpoint", 2) => Ok(Def::Midpoint(obj(args[0])?, obj(args[1])?)),
        ("Distance", 2) => Ok(Def::Distance(obj(args[0])?, obj(args[1])?)),
        ("Perpendicular" | "PerpendicularLine" | "OrthogonalLine", 2) => Ok(Def::Perpendicular {
            through: obj(args[0])?,
            to: obj(args[1])?,
        }),
        ("Parallel", 2) => Ok(Def::Parallel {
            through: obj(args[0])?,
            to: obj(args[1])?,
        }),
        ("Circle", 2) => {
            let center = obj(args[0])?;
            let second = obj(args[1])?;
            Ok(if c.kind(second) == Kind::Number {
                Def::CircleRadius {
                    center,
                    radius: second,
                }
            } else {
                Def::CircleThrough {
                    center,
                    through: second,
                }
            })
        }
        ("Intersect", 2 | 3) => {
            let first = obj(args[0])?;
            let second = obj(args[1])?;
            let which = match args.get(2) {
                None => 0,
                Some(&"1") => 0,
                Some(&"2") => 1,
                Some(other) => {
                    return Err(format!("intersection number must be 1 or 2, got `{other}`"))
                }
            };
            match (c.kind(first), c.kind(second)) {
                (Kind::Line, Kind::Line) => Ok(Def::IntersectLines(first, second)),
                (Kind::Line, Kind::Circle) => Ok(Def::IntersectLineCircle {
                    line: first,
                    circle: second,
                    which,
                }),
                (Kind::Circle, Kind::Line) => Ok(Def::IntersectLineCircle {
                    line: second,
                    circle: first,
                    which,
                }),
                (Kind::Circle, Kind::Circle) => Ok(Def::IntersectCircles {
                    c1: first,
                    c2: second,
                    which,
                    special: None,
                }),
                (a, b) => Err(format!("cannot intersect a {a} with a {b}")),
            }
        }
        (
            "Line" | "Segment" | "Midpoint" | "Distance" | "Perpendicular" | "PerpendicularLine"
            | "OrthogonalLine" | "Parallel" | "Circle" | "Intersect",
            n,
        ) => Err(format!("`{func}` does not take {n} argument(s)")),
        _ => Err(format!("unknown command `{func}`")),
    }
}

/// Looks up an object by its label.
pub(crate) fn object(c: &Construction, label: &str) -> Result<Id, String> {
    c.id(label)
        .ok_or_else(|| format!("unknown object `{label}`"))
}

fn parse_number(s: &str) -> Result<f64, String> {
    crate::expr::plain_number(s).ok_or_else(|| format!("`{s}` is not a number"))
}

/// Names GeoGebra never gives to an object of these forms: `x = 5` and
/// `pi = 3` are equations there, and `sin = 3` is renamed `sin_{1}` (all
/// observed with the harness). Command names such as `Mod` and the letters
/// `e` and `i` are ordinary labels. GeoGebra also reads `inf = ...`,
/// `Infinity = ...`, `NaN = ...` and `Undefined = ...` as equations; here
/// they stay labels, which the polygon forms rely on (see DIFFERENCES.md),
/// while an expression still reads these words as `∞` and `?`.
fn check_label(name: &str) -> Result<(), String> {
    const RESERVED: &[&str] = &[
        "x",
        "y",
        "z",
        "pi",
        "Pi",
        "\u{3c0}",
        "\u{212f}",
        "euler_gamma",
        "deg",
        "rad",
        "true",
        "True",
        "false",
        "False",
    ];
    if RESERVED.contains(&name) || crate::expr::is_function_name(name) && !is_command_name(name) {
        return Err(format!("`{name}` is a reserved name"));
    }
    Ok(())
}

/// Commands among the built-in names (`Mod`, `Max`, ...): upper case.
fn is_command_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// `$point(x, y)` and `$vector(x, y)`: a free point or a free vector whatever
/// the label looks like. `None` for any other right-hand side.
fn parse_forced(rhs: &str) -> Option<Result<Def, String>> {
    let (func, args) = split_call(rhs)?;
    if func != "$point" && func != "$vector" {
        return None;
    }
    if args.len() != 2 {
        return Some(Err(format!("`{func}` needs two coordinates")));
    }
    let (x, y) = match (parse_number(args[0]), parse_number(args[1])) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
    };
    Some(Ok(if func == "$point" {
        Def::FreePoint { x, y }
    } else {
        Def::Vector(VectorDef::Free { x, y })
    }))
}

/// Index of the `}` that closes the `{` at the start of `s`. Labels such as
/// `t_{1}` contain balanced braces of their own.
fn closing_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits at the commas that are outside braces.
fn split_slots(s: &str) -> Vec<&str> {
    let mut slots = Vec::new();
    let (mut depth, mut start) = (0usize, 0);
    for (i, ch) in s.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                slots.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    slots.push(s[start..].trim());
    slots
}

/// A label for an output slot or a plain line: an identifier, or the indexed
/// form `prefix_{inner}` where `inner` is one or more letters, digits, `_` or
/// `'` (`t_{1}`, `v_{max}`).
fn is_output_label(s: &str) -> bool {
    match s.strip_suffix('}').and_then(|t| t.split_once("_{")) {
        Some((prefix, inner)) => {
            is_identifier(prefix)
                && !inner.is_empty()
                && inner
                    .chars()
                    .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '\'')
        }
        None => is_identifier(s),
    }
}

/// Whether `rhs` (`Command(args)`) is a command that creates several objects
/// and, if so, how many of them the kernel makes now (or why it cannot). `None`
/// for a command that creates one object, and for anything that is not a call.
/// A saved file uses it to choose between `name = ...` and `{a, b} = ...`.
pub fn multi_output_count(c: &Construction, rhs: &str) -> Option<Result<usize, String>> {
    parse_multi(c, rhs.trim()).map(|r| r.map(|defs| defs.len()))
}

/// `{t_{1}, t_{2}} = Tangent(P, k)`: names the outputs of a command that
/// creates several objects. The slots follow GeoGebra's output order; an
/// empty slot is an output that is not created. Nothing is created when the
/// line fails. Returns the id of the first object created.
fn exec_labelled(c: &mut Construction, line: &str) -> Result<Option<Id>, String> {
    let close = closing_brace(line).ok_or("missing closing `}` after the output labels")?;
    let slots = split_slots(&line[1..close]);
    let rhs = line[close + 1..]
        .trim_start()
        .strip_prefix('=')
        .ok_or("expected `=` after the output labels")?
        .trim();
    // The command decides how many outputs it has; a longer list of labels is
    // refused before any label is looked at, so the work is bounded by the
    // command, not by the length of the line.
    let Some(defs) = parse_multi(c, rhs) else {
        // An unknown command or a bad argument is the better message.
        return Err(match parse_def(c, rhs) {
            Err(e) => e,
            Ok(_) => "the command does not create several objects, write `name = ...`".into(),
        });
    };
    let defs = defs?;
    // A command can make fewer outputs than GeoGebra's algorithm has (the
    // second tangent from a point on a circle is undefined at creation and
    // never created); slots for outputs that are not created may stay empty.
    if defs.len() > slots.len() || slots[defs.len()..].iter().any(|s| !s.is_empty()) {
        return Err(format!(
            "the command creates {} outputs, but {} labels are given",
            defs.len(),
            slots.len()
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for slot in &slots {
        if slot.is_empty() {
            continue;
        }
        if !is_output_label(slot) {
            return Err(format!("`{slot}` is not a valid object name"));
        }
        check_label(slot)?;
        if !seen.insert(*slot) {
            return Err(format!("the label `{slot}` is used twice"));
        }
        if c.id(slot).is_some() {
            return Err(format!("object `{slot}` already exists"));
        }
    }
    if seen.is_empty() {
        return Err("no output is given a label".into());
    }
    for (slot, def) in slots.iter().zip(&defs) {
        if !slot.is_empty() {
            probe(c, def)?;
        }
    }
    // All or nothing: a failure after the first object removes it again.
    let at = c.checkpoint();
    let mut first = None;
    for (slot, def) in slots.iter().zip(defs) {
        if !slot.is_empty() {
            match c.add(slot, def) {
                Ok(id) => {
                    first.get_or_insert(id);
                }
                Err(e) => {
                    c.discard_since(at);
                    return Err(e.to_string());
                }
            }
        }
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Value;

    #[test]
    fn vector_labels_follow_string_util_is_lower_case() {
        assert!(is_vector_label("a_1"));
        assert!(is_vector_label("\u{3b1}")); // Greek small alpha
        assert!(is_vector_label("\u{142}k")); // Latin small l with stroke
        assert!(is_vector_label("\u{b7}")); // a letter for GeoGebra, unchanged by lower case
        assert!(!is_vector_label("A_1"));
        assert!(!is_vector_label("_p"));
        assert!(!is_vector_label("\u{141}")); // Latin capital L with stroke
        assert!(!is_vector_label("\u{1d44e}")); // mathematical italic small a, outside the BMP
        assert!(!is_vector_label("\u{2170}")); // small roman numeral one, a letter number
        assert!(!is_vector_label(""));
    }

    #[test]
    fn builds_a_construction_from_text() {
        let c = load(
            "# two circles\n\
             A = (0, 0)\n\
             B = (8, 0)\n\
             r = 5\n\
             k1 = Circle(A, r)\n\
             k2 = Circle(B, r)\n\
             P = Intersect(k1, k2)      # first point\n\
             Q = Intersect(k1, k2, 2)\n\
             d = Distance(P, Q)\n",
        )
        .unwrap();

        assert_eq!(c.len(), 8);
        assert_eq!(c.value(c.id("P").unwrap()), Value::Point { x: 4.0, y: 3.0 });
        assert_eq!(c.value(c.id("d").unwrap()), Value::Number(6.0));
    }

    #[test]
    fn geogebra_parallel_and_perpendicular_forms() {
        let c = load(
            "A = (0, 0)\nB = (4, 0)\nP = (1, 3)\n\
             l = Line(A, B)\ns = Segment(A, B)\n\
             p = Line(P, l)\nq = Line(P, s)\n\
             n = PerpendicularLine(P, l)\nm = Line(A, P)\n",
        )
        .unwrap();
        let v = |n: &str| c.value(c.id(n).unwrap());
        // GeoGebra's raw coefficients: those of l, (0, 4, 0), moved through P.
        assert_eq!(
            v("p"),
            Value::Line {
                a: 0.0,
                b: 4.0,
                c: -12.0
            }
        );
        assert_eq!(v("q"), v("p"));
        assert_eq!(c.kind(c.id("n").unwrap()), Kind::Line);
        let old =
            load("A = (0, 0)\nB = (4, 0)\nP = (1, 3)\nl = Line(A, B)\nn = Perpendicular(P, l)\n")
                .unwrap();
        assert_eq!(old.value(old.id("n").unwrap()), v("n"));
        // Two points still make a plain line.
        assert!(matches!(c.def(c.id("m").unwrap()), Def::Line(..)));
        assert!(matches!(c.def(c.id("p").unwrap()), Def::Parallel { .. }));
    }

    #[test]
    fn reports_the_failing_line() {
        let err = load("A = (0, 0)\nl = Line(A, Z)\n").err().unwrap();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("unknown object `Z`"));

        let err = load("A = (0, 0)\nB = (1, 1)\nx = Intersect(A, B)\n")
            .err()
            .unwrap();
        assert_eq!(err.line, 3);

        let err = load("A = Foo(1)\n").err().unwrap();
        assert!(err.message.contains("unknown command"));

        let err = load("A = (0, 0)\nB = (1, 1)\nl = Line(A)\n").err().unwrap();
        assert!(err.message.contains("does not take 1"));
    }

    #[test]
    fn labels_several_outputs_like_geogebra() {
        let mut c = load(
            "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\nB = (4, 0)\nC = (0, 1)\n\
             l = Line(A, B)\nm = Line(A, C)\n",
        )
        .unwrap();
        // two defined outputs: indexed labels, the plain name is not used
        let first = exec(&mut c, "t = Tangent(P, k)").unwrap().unwrap();
        assert_eq!(c.label(first), "t_{1}");
        assert!(c.id("t_{2}").is_some() && c.id("t").is_none());
        // the prefix is the name up to the first `_`
        exec(&mut c, "w_7 = AngularBisector(l, m)").unwrap();
        assert!(c.id("w_{1}").is_some() && c.id("w_{2}").is_some());
        // a short label `b_1` blocks the index 1
        exec(&mut c, "b_1 = (1, 1)").unwrap();
        exec(&mut c, "b = AngularBisector(l, m)").unwrap();
        assert!(c.id("b_{1}").is_none());
        assert!(c.id("b_{2}").is_some() && c.id("b_{3}").is_some());
        // an existing name is an error
        let e = exec(&mut c, "k = Tangent(P, k)").unwrap_err();
        assert!(e.contains("already exists"), "{e}");
    }

    #[test]
    fn a_single_defined_output_gets_the_plain_label() {
        let c = load(
            "A = (0, 0)\nr = 5\nk = Circle(A, r)\nP = (3, 4)\nt = Tangent(P, k)\n\
             B = (4, 0)\nC = (0, 2)\nD = (-4, 2)\nl = Line(A, B)\nm = Line(C, D)\n\
             b = AngularBisector(l, m)\n",
        )
        .unwrap();
        // P on the circle: GeoGebra labels only the first tangent
        assert!(c.id("t").is_some() && c.id("t_{1}").is_none() && c.id("t_{2}").is_none());
        // parallel lines with opposite directions: only the second bisector
        // is defined, and it gets the plain label
        let b = c.id("b").unwrap();
        assert!(matches!(
            c.def(b),
            Def::Lines(crate::algo::lines::LineDef::AngleBisectorLines { which: 1, .. })
        ));
        assert_eq!(
            c.value(b),
            Value::Line {
                a: 0.0,
                b: 1.0,
                c: -1.0
            }
        );
    }

    #[test]
    fn explicit_output_labels() {
        let mut c = load(
            "A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\nB = (4, 0)\nC = (0, 1)\n\
             l = Line(A, B)\nm = Line(A, C)\n",
        )
        .unwrap();
        let first = exec(&mut c, "{t_{1}, t_{2}} = Tangent(P, k)")
            .unwrap()
            .unwrap();
        assert_eq!(c.label(first), "t_{1}");
        assert!(c.id("t_{2}").is_some() && c.id("t").is_none());
        // any labels, an empty slot creates nothing
        exec(&mut c, "{u, } = Tangent(P, k)").unwrap();
        assert!(c.id("u").is_some() && c.len() == 11);
        exec(&mut c, "{, w} = Tangent(P, k)").unwrap();
        assert!(matches!(
            c.def(c.id("w").unwrap()),
            Def::Lines(crate::algo::lines::LineDef::TangentPoint { which: 1, .. })
        ));
        // the labels do not depend on how many outputs are defined
        exec(&mut c, "{b1, b2} = AngularBisector(l, m)").unwrap();
        assert!(c.id("b1").is_some() && c.id("b2").is_some());
        // the second tangent from a point on the circle is never created: its slot stays empty;
        // a label for it is an error (the limit stated at the top of this file)
        exec(&mut c, "T = (2, 0)").unwrap();
        exec(&mut c, "{o, } = Tangent(T, k)").unwrap();
        exec(&mut c, "T2 = (0, 2)").unwrap();
        exec(&mut c, "{o2, , } = Tangent(T2, k)").unwrap();
        let n = c.len();
        let e = exec(&mut c, "{o3, o4} = Tangent(T, k)").unwrap_err();
        assert!(e.contains("creates 1 outputs, but 2 labels"), "{e}");
        assert_eq!(c.len(), n);
        assert!(c.id("o").is_some() && c.id("o2").is_some());
        // an output that is undefined at creation is still created when it has a label
        exec(&mut c, "Q = (4, 1)").unwrap();
        exec(&mut c, "{s1, s2} = Tangent(A, k)").unwrap();
        assert!(c.id("s1").is_some() && c.id("s2").is_some());
    }

    #[test]
    fn explicit_output_labels_fail_cleanly() {
        let mut c = load("A = (0, 0)\nr = 2\nk = Circle(A, r)\nP = (5, 1)\n").unwrap();
        let n = c.len();
        for line in [
            "{t_{1}, t_{2} = Tangent(P, k)",
            "{t_{1}, t_{2}} Tangent(P, k)",
            "{t_{1}} = Tangent(P, k)",
            "{t_{1}, t_{2}, t_{3}} = Tangent(P, k)",
            "{, , t} = Tangent(P, k)",
            "{a, a} = Tangent(P, k)",
            "{A, b} = Tangent(P, k)",
            "{, } = Tangent(P, k)",
            "{a, 1b} = Tangent(P, k)",
            "{a, b} = Circle(A, r)",
            "{a, b} = (1, 2)",
            "{a, b} = Tangent(P, Z)",
            "{a, b} = Foo(P)",
            "{a, b} = Polygon(A, A, A)",
            "{}",
            "{",
            "}{",
        ] {
            assert!(exec(&mut c, line).is_err(), "{line}");
            assert_eq!(c.len(), n, "{line}");
        }
        let e = exec(&mut c, "{a, b} = Foo(P)").unwrap_err();
        assert!(e.contains("unknown command `Foo`"), "{e}");
        let e = exec(&mut c, "{a, b} = Circle(A, r)").unwrap_err();
        assert!(e.contains("does not create several"), "{e}");
    }

    #[test]
    fn forced_point_and_vector_forms() {
        let mut c = Construction::new();
        exec(&mut c, "a = $point(1, 2)").unwrap();
        exec(&mut c, "B = $vector(3, 4)").unwrap();
        exec(&mut c, "c = (5, 6)").unwrap();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Point { x: 1.0, y: 2.0 });
        assert_eq!(
            c.value(c.id("B").unwrap()),
            Value::Vector { x: 3.0, y: 4.0 }
        );
        assert_eq!(c.kind(c.id("c").unwrap()), Kind::Vector);
        // a lower-case point can be moved like any free point
        c.move_point(c.id("a").unwrap(), 7.0, 8.0).unwrap();
        assert_eq!(c.value(c.id("a").unwrap()), Value::Point { x: 7.0, y: 8.0 });
        assert!(exec(&mut c, "d = $point(1)").is_err());
        assert!(exec(&mut c, "d = $point(1, x)").is_err());
        assert!(exec(&mut c, "a = $vector(1, 2)").is_err());
    }
}
