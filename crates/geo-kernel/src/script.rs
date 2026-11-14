// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/LabelManager.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/Construction.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! A small input-bar style language, one definition per line:
//!
//! ```text
//! A = (0, 0)              # free point
//! r = 3                   # free number
//! l = Line(A, B)
//! m = Line(P, l)          # GeoGebra form: parallel to l (a line or segment) through P
//! n = PerpendicularLine(P, l)  # GeoGebra name for `Perpendicular`
//! k = Circle(A, B)        # centre A, through B
//! m = Circle(A, r)        # centre A, radius r
//! P = Intersect(l, k, 2)  # second intersection (1 is the default)
//! t = Tangent(P, k)       # two tangents, labelled t_{1} and t_{2} as in GeoGebra
//! ```

use crate::algo::conic::ConicDef;
use crate::algo::lines::LineDef;
use crate::algo::measure::MeasureDef;
use crate::algo::transform::TransformDef;
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
    let (name, rhs) = line.split_once('=').ok_or("expected `name = definition`")?;
    let name = name.trim();
    if !is_identifier(name) {
        return Err(format!("`{name}` is not a valid object name"));
    }
    let rhs = rhs.trim();
    if let Some(defs) = parse_multi(c, rhs) {
        return add_outputs(c, name, defs?).map(Some);
    }
    let def = parse_def(c, rhs)?;
    c.add(name, def).map(Some).map_err(|e| e.to_string())
}

/// `Name(args)` split into the name and the trimmed arguments; `None` for
/// anything else (a point literal, a number, a malformed line).
fn split_call(rhs: &str) -> Option<(&str, Vec<&str>)> {
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

/// Adds the outputs of one command and labels them as GeoGebra's
/// `LabelManager.setLabels` does for `name = Command(...)`: when two or more
/// outputs are defined, every output gets the next free indexed label
/// (`t_{1}`, `t_{2}`, ...); when only one is defined, it gets `name` and the
/// others get no label, so they are not created here. When none is defined,
/// GeoGebra suggests `name` for all of them; only the first is created.
/// Returns the id of the first object created.
fn add_outputs(c: &mut Construction, name: &str, defs: Vec<Def>) -> Result<Id, String> {
    if c.id(name).is_some() {
        return Err(format!("object `{name}` already exists"));
    }
    let mut defined = Vec::with_capacity(defs.len());
    for def in &defs {
        defined.push(probe(c, def)?);
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
    for def in defs {
        let label = index_label(c, prefix);
        let id = c.add(&label, def).map_err(|e| e.to_string())?;
        first.get_or_insert(id);
    }
    first.ok_or_else(|| "the command creates no object".into())
}

/// `Construction.buildIndexedLabel`: the first `p_{n}` that is free, where
/// for `n < 10` the short form `p_n` must be free too.
fn index_label(c: &Construction, prefix: &str) -> String {
    let mut n = 1;
    loop {
        let long = format!("{prefix}_{{{n}}}");
        if c.id(&long).is_none() && (n >= 10 || c.id(&format!("{prefix}_{n}")).is_none()) {
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
        Value::Undefined => false,
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(ch) if ch.is_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '\'')
}

fn parse_def(c: &Construction, rhs: &str) -> Result<Def, String> {
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
    if let Ok(v) = rhs.parse::<f64>() {
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
    // built-in command (for example `Circle` with three points).
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
    s.parse::<f64>()
        .map_err(|_| format!("`{s}` is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Value;

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
        assert_eq!(
            v("p"),
            Value::Line {
                a: 0.0,
                b: 1.0,
                c: -3.0
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
}
