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
//! ```

use crate::kernel::{Construction, Def, Id, Kind};
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
    let def = parse_def(c, rhs.trim())?;
    c.add(name, def).map(Some).map_err(|e| e.to_string())
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
    let obj = |s: &str| c.id(s).ok_or_else(|| format!("unknown object `{s}`"));

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
        ("Perpendicular" | "PerpendicularLine", 2) => Ok(Def::Perpendicular {
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
            | "Parallel" | "Circle" | "Intersect",
            n,
        ) => Err(format!("`{func}` does not take {n} argument(s)")),
        _ => Err(format!("unknown command `{func}`")),
    }
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
}
