// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectSingle.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdIntersect.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Objects computed from a general conic ([`Kind::Conic`]): its
//! intersection points with a line.
//!
//! They read the conic's [`ConicData`](super::conic_nd::ConicData) record through
//! [`Construction::conic`], so they work on every object that has one.

use super::{check_index, Group, Parents, LINE};
use crate::kernel::{Construction, Error, Id, Kind, Value};

const CONIC: &[Kind] = &[Kind::Conic];

/// Objects computed from a general conic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicPartDef {
    /// `Intersect(g, k, n)` and `Intersect(k, g, n)`: `which` is 0 or 1,
    /// GeoGebra's intersection 1 or 2 of a line and a conic.
    IntersectLine { line: Id, conic: Id, which: u8 },
}

impl Group for ConicPartDef {
    fn kind(&self) -> Kind {
        match self {
            ConicPartDef::IntersectLine { .. } => Kind::Point,
        }
    }

    fn parents(&self) -> Parents {
        match *self {
            ConicPartDef::IntersectLine { line, conic, .. } => Parents::of(&[line, conic]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            ConicPartDef::IntersectLine { line, conic, which } => {
                c.expect(line, LINE)?;
                c.expect(conic, CONIC)?;
                check_index(which)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            ConicPartDef::IntersectLine { line, conic, which } => {
                super::intersect::line_general_conic(c, line, conic, which)
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        match (func, args.len()) {
            ("Intersect", 2 | 3) => {
                let first = c.id(args[0])?;
                let second = c.id(args[1])?;
                let (line, conic) = match (c.kind(first), c.kind(second)) {
                    (Kind::Line, Kind::Conic) => (first, second),
                    (Kind::Conic, Kind::Line) => (second, first),
                    _ => return None,
                };
                Some(
                    parse_index(args.get(2).copied()).map(|which| ConicPartDef::IntersectLine {
                        line,
                        conic,
                        which,
                    }),
                )
            }
            _ => None,
        }
    }
}

/// The optional intersection number `1` or `2` as an index 0 or 1.
fn parse_index(arg: Option<&str>) -> Result<u8, String> {
    match arg {
        None | Some("1") => Ok(0),
        Some("2") => Ok(1),
        Some(other) => Err(format!("intersection number must be 1 or 2, got `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use crate::kernel::{Construction, Value};
    use crate::script::load;

    fn xy(c: &Construction, label: &str) -> Option<(f64, f64)> {
        match c.value(c.id(label).unwrap()) {
            Value::Point { x, y } => Some((x, y)),
            _ => None,
        }
    }

    fn close(p: Option<(f64, f64)>, x: f64, y: f64) -> bool {
        p.is_some_and(|(px, py)| (px - x).abs() < 1e-9 && (py - y).abs() < 1e-9)
    }

    const ELLIPSE: &str = "F = (-3, 0)\nG = (3, 0)\nP = (0, 4)\ne = Ellipse(F, G, P)\n";

    #[test]
    fn a_line_meets_an_ellipse_in_two_points() {
        let c = load(&format!(
            "{ELLIPSE}A = (0, -10)\nB = (0, 10)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(e, g, 2)\n"
        ))
        .unwrap();
        let (s, t) = (xy(&c, "S"), xy(&c, "T"));
        assert!(close(s, 0.0, 4.0) || close(s, 0.0, -4.0));
        assert!(close(t, 0.0, 4.0) || close(t, 0.0, -4.0));
        assert_ne!(s, t);
    }

    #[test]
    fn a_tangent_point_is_only_intersection_one_and_a_passing_line_none() {
        let mut c = load(&format!(
            "{ELLIPSE}A = (-1, 4)\nB = (1, 4)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n"
        ))
        .unwrap();
        assert!(close(xy(&c, "S"), 0.0, 4.0));
        assert_eq!(xy(&c, "T"), None);
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        c.move_point(a, -1.0, 5.0).unwrap();
        c.move_point(b, 1.0, 5.0).unwrap();
        assert_eq!(xy(&c, "S"), None);
        assert_eq!(xy(&c, "T"), None);
    }

    #[test]
    fn a_line_parallel_to_the_axis_meets_a_parabola_once_as_intersection_one() {
        // y = x^2 / 4: focus (0, 1), directrix y = -1
        let c = load(
            "F = (0, 1)\nA = (0, -1)\nB = (1, -1)\nd = Line(A, B)\np = Parabola(F, d)\nC = (2, 0)\nD = (2, 5)\ng = Line(C, D)\nS = Intersect(g, p, 1)\nT = Intersect(g, p, 2)\nE = (2, 5)\nH = (2, 0)\nh = Line(E, H)\nU = Intersect(h, p, 1)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 2.0, 1.0));
        assert_eq!(xy(&c, "T"), None);
        assert!(close(xy(&c, "U"), 2.0, 1.0));
    }

    #[test]
    fn a_secant_of_a_double_line_meets_it_once() {
        let c = load(
            "F = (-3, 0)\nG = (3, 0)\na = 3\ne = Ellipse(F, G, a)\nA = (1, -2)\nB = (1, 2)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 1.0, 0.0));
        assert_eq!(xy(&c, "T"), None);
    }

    #[test]
    fn a_point_on_line_and_conic_by_construction_comes_first() {
        for (first, second) in [("A", "X"), ("X", "A")] {
            let mut c = load(&format!(
                "A = (1, 0)\nB = (0, 1)\nC = (-1, 0)\nD = (0, -1)\nE = (0.6, 0.8)\nk = Conic(A, B, C, D, E)\nX = (-2, 1)\ng = Line({first}, {second})\nS = Intersect(g, k, 1)\nT = Intersect(g, k, 2)\n"
            ))
            .unwrap();
            assert!(close(xy(&c, "S"), 1.0, 0.0), "{first} {second}");
            assert!(xy(&c, "T").is_some());
            assert!(!close(xy(&c, "T"), 1.0, 0.0));
            // the point stays first when the line turns
            let x = c.id("X").unwrap();
            c.move_point(x, 3.0, 2.0).unwrap();
            assert!(close(xy(&c, "S"), 1.0, 0.0), "{first} {second}");
        }
    }

    #[test]
    fn rebuilt_intersections_have_the_same_values() {
        let script = format!(
            "{ELLIPSE}A = (-6, -1)\nB = (5, 2)\ng = Line(A, B)\nS = Intersect(g, e, 1)\nT = Intersect(g, e, 2)\n"
        );
        let mut c = load(&script).unwrap();
        let a = c.id("A").unwrap();
        c.move_point(a, -4.0, 3.0).unwrap();
        c.move_point(a, -6.0, -1.0).unwrap();
        let fresh = load(&script).unwrap();
        for label in ["S", "T"] {
            assert_eq!(xy(&c, label), xy(&fresh, label), "{label}");
        }
    }

    #[test]
    fn an_undefined_conic_makes_the_intersections_undefined() {
        let mut c = load(
            "F = (-3, 0)\nG = (3, 0)\na = 5\ne = Ellipse(F, G, a)\nf = 2\nk = Dilate(e, f)\nA = (0, -20)\nB = (0, 20)\ng = Line(A, B)\nS = Intersect(g, k, 1)\n",
        )
        .unwrap();
        assert!(close(xy(&c, "S"), 0.0, 8.0) || close(xy(&c, "S"), 0.0, -8.0));
        let f = c.id("f").unwrap();
        c.set_number(f, 0.0).unwrap();
        assert_eq!(xy(&c, "S"), None);
        c.set_number(f, 2.0).unwrap();
        assert!(xy(&c, "S").is_some());
    }
}
