// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLength.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLengthSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoRadius.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdPerimeter.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdCircumference.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoCircumferenceConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdArea.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAreaPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolygon.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSlope.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoSlope.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdDistance.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistancePointObject.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoClosestPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoDistanceLineLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdAffineRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoAffineRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdCrossRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/AlgoCrossRatio.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/matrix/Coords.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAnglePoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoJoinPointsSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoOrthoLinePointLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoAngle.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Numbers measured on objects: length, radius, circumference, area, slope,
//! distances, affine and cross ratio, and the angle of three points.
//!
//! Zero tests on lines use GeoGebra's raw coefficients (see [`raw_line`]).
//!
//! GeoGebra decides "is this zero" with the absolute tolerance
//! [`STANDARD_PRECISION`]; the same tests are used here so that degenerate
//! cases switch at the same threshold.

use super::{Group, Parents, CIRCLE, LINE_LIKE, POINT};
use crate::kernel::{Construction, Def, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::STANDARD_PRECISION;
use std::f64::consts::PI;

const SEGMENT: &[Kind] = &[Kind::Segment];

/// Numbers measured on objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureDef {
    /// `Length(segment)`.
    Length(Id),
    /// `Radius(circle)`.
    Radius(Id),
    /// `Circumference(circle)` and `Perimeter(circle)`.
    Circumference(Id),
    /// `Area(circle)`.
    CircleArea(Id),
    /// `Area(P, Q, R)`: area of the triangle.
    Area3(Id, Id, Id),
    /// `Area(P, Q, R, S)`: area of the quadrilateral through the points in order.
    Area4(Id, Id, Id, Id),
    /// `Slope(line or segment)`.
    Slope(Id),
    /// `Distance(point, line or segment)` in either order of the arguments.
    DistancePointPath { point: Id, path: Id },
    /// `Distance(line, line)`; segments count as the lines they lie on.
    DistanceLines(Id, Id),
    /// `AffineRatio(A, B, C)`.
    AffineRatio(Id, Id, Id),
    /// `CrossRatio(A, B, C, D)`.
    CrossRatio(Id, Id, Id, Id),
    /// `Angle(A, B, C)`: the angle at `B`, counter-clockwise from `BA` to `BC`.
    Angle(Id, Id, Id),
}

/// `-eps < v < eps` (GeoGebra's `DoubleUtil.isZero`).
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// GeoGebra's `DoubleUtil.isEqual(a, b)`.
fn is_equal(a: f64, b: f64) -> bool {
    a == b || ((a - STANDARD_PRECISION) <= b && b <= (a + STANDARD_PRECISION))
}

/// GeoGebra's `MyMath.length`: no overflow, no underflow.
fn my_length(x: f64, y: f64) -> f64 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        ay
    } else if ay == 0.0 {
        ax
    } else if ax > ay {
        let t = ay / ax;
        ax * (1.0 + t * t).sqrt()
    } else {
        let t = ax / ay;
        ay * (1.0 + t * t).sqrt()
    }
}

/// Radius of a circle as GeoGebra reports it: a radius below the tolerance
/// makes the conic a single point of radius 0.
fn circle_radius(c: &Construction, id: Id) -> Option<f64> {
    let (_, _, r) = c.circle(id)?;
    Some(if is_zero(r) { 0.0 } else { r })
}

/// Length of a segment; GeoGebra sets lengths below the tolerance to 0.
fn segment_length(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let length = my_length(x2 - x1, y2 - y1);
    if is_zero(length) {
        0.0
    } else {
        length
    }
}

/// The coefficients `(x, y, z)` of a line or a segment as GeoGebra holds
/// them, which are not normalised: the algorithm that makes the line decides
/// the scale, and GeoGebra's zero tests depend on it.
///
/// Reproduced exactly for `Line(P, Q)` and `Segment(P, Q)` (the cross product
/// of the end points), for a parallel (the coefficients of the parent) and
/// for a perpendicular (the rotated coefficients of the parent, rescaled by
/// powers of two when they leave `[1e-2, 1e7]`). Lines made by another group
/// are used normalised, which is their scale only by chance.
fn raw_line(c: &Construction, id: Id) -> Option<(f64, f64, f64)> {
    let (a, b, z) = match c.value(id) {
        Value::Undefined => return None,
        Value::Line { a, b, c } => (a, b, c),
        Value::Segment { x1, y1, x2, y2 } => return Some((y1 - y2, x2 - x1, x1 * y2 - y1 * x2)),
        _ => return None,
    };
    match c.def(id) {
        Def::Line(p, q) => {
            let (p, q) = (c.point(p)?, c.point(q)?);
            Some((p.1 - q.1, q.0 - p.0, p.0 * q.1 - p.1 * q.0))
        }
        Def::Parallel { through, to } => {
            let (lx, ly, _) = raw_line(c, to)?;
            let p = c.point(through)?;
            Some((lx, ly, p.0 * -lx - p.1 * ly))
        }
        Def::Perpendicular { through, to } => {
            let (lx, ly, _) = raw_line(c, to)?;
            let p = c.point(through)?;
            Some(rescale((-ly, lx, p.0 * ly - p.1 * lx), 10e6, 1e-2))
        }
        _ => Some((a, b, z)),
    }
}

/// `GeoLine.getNormalizedCoefficients`: scales by powers of two so that some
/// coefficient lies in `[min, max]`.
fn rescale(v: (f64, f64, f64), max: f64, min: f64) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = v;
    if is_zero(x) && is_zero(y) && is_zero(z) || !(x.is_finite() && y.is_finite() && z.is_finite())
    {
        return v;
    }
    while x.abs() < min && y.abs() < min && z.abs() < min {
        (x, y, z) = (x * 2.0, y * 2.0, z * 2.0);
    }
    while x.abs() > max && y.abs() > max && z.abs() > max {
        (x, y, z) = (x / 2.0, y / 2.0, z / 2.0);
    }
    (x, y, z)
}

/// `GeoLine.getSlope`: undefined for a vertical line.
fn slope(x: f64, y: f64) -> Option<f64> {
    if is_zero(y) {
        None
    } else {
        Some(-x / y)
    }
}

/// `GeoPoint.collinearND` for points of the plane: the cross product of
/// `A - B` and `A - C` vanishes.
fn collinear(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let (bx, by) = (a.0 - b.0, a.1 - b.1);
    let (cx, cy) = (a.0 - c.0, a.1 - c.1);
    is_zero(bx * cy - by * cx)
}

fn same_point(p: (f64, f64), q: (f64, f64)) -> bool {
    is_equal(p.0, q.0) && is_equal(p.1, q.1)
}

/// `GeoPoint.affineRatio`: `lambda` with `pt2 = start + lambda * (pt1 - start)`.
fn affine_ratio(start: (f64, f64), pt1: (f64, f64), pt2: (f64, f64)) -> f64 {
    let abx = pt1.0 - start.0;
    let aby = pt1.1 - start.1;
    if abx.abs() > aby.abs() {
        (pt2.0 - start.0) / abx
    } else if aby.abs() > 0.0 {
        (pt2.1 - start.1) / aby
    } else {
        f64::NAN
    }
}

/// `DoubleUtil.convertToAngleValue`: an angle in `[0, 2 pi]`.
fn angle_value(val: f64) -> f64 {
    const TWO_PI: f64 = 2.0 * PI;
    if val > STANDARD_PRECISION && val < TWO_PI {
        return val;
    }
    let mut value = val % TWO_PI;
    if is_zero(value) {
        value = if val < 1.0 { 0.0 } else { TWO_PI };
    } else if value < 0.0 {
        value += TWO_PI;
    }
    value
}

/// Twice the signed area of the polygon through `pts` (`AlgoPolygon.calcAreaWithSign`).
fn polygon_area(pts: &[(f64, f64)]) -> f64 {
    let last = pts.len() - 1;
    let mut sum = 0.0;
    for i in 0..last {
        sum += pts[i].0 * pts[i + 1].1 - pts[i + 1].0 * pts[i].1;
    }
    sum += pts[last].0 * pts[0].1 - pts[0].0 * pts[last].1;
    sum / 2.0
}

/// Distance of a point to a segment: to the closest point of the segment.
fn point_segment_distance(p: (f64, f64), x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let length = segment_length(x1, y1, x2, y2);
    if length == 0.0 {
        return my_length(p.0 - x1, p.1 - y1);
    }
    let (dx, dy) = (x2 - x1, y2 - y1);
    let (px, py) = (p.0 - x1, p.1 - y1);
    let t = (px * dx + py * dy) / (dx * dx + dy * dy);
    if t < 0.0 {
        my_length(px, py)
    } else if t > 1.0 {
        my_length(p.0 - x2, p.1 - y2)
    } else {
        (px * dy - py * dx).abs() / my_length(dx, dy)
    }
}

/// `GeoLine.distance(GeoLine)` for `this` = `g` and the argument `h`:
/// zero unless the lines are parallel.
fn line_line_distance(g: (f64, f64, f64), h: (f64, f64, f64)) -> f64 {
    if is_zero(h.0 * g.1 - h.1 * g.0) {
        // A point of h.
        let (px, py) = if h.0.abs() > h.1.abs() {
            (-h.2 / h.0, 0.0)
        } else {
            (0.0, -h.2 / h.1)
        };
        ((g.0 * px + g.1 * py + g.2) / my_length(g.0, g.1)).abs()
    } else {
        0.0
    }
}

impl Group for MeasureDef {
    fn kind(&self) -> Kind {
        Kind::Number
    }

    fn parents(&self) -> Parents {
        match *self {
            MeasureDef::Length(a)
            | MeasureDef::Radius(a)
            | MeasureDef::Circumference(a)
            | MeasureDef::CircleArea(a)
            | MeasureDef::Slope(a) => Parents::of(&[a]),
            MeasureDef::DistancePointPath { point, path } => Parents::of(&[point, path]),
            MeasureDef::DistanceLines(g, h) => Parents::of(&[g, h]),
            MeasureDef::Area3(a, b, c)
            | MeasureDef::AffineRatio(a, b, c)
            | MeasureDef::Angle(a, b, c) => Parents::of(&[a, b, c]),
            MeasureDef::Area4(a, b, c, d) | MeasureDef::CrossRatio(a, b, c, d) => {
                Parents::of(&[a, b, c, d])
            }
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            MeasureDef::Length(s) => c.expect(s, SEGMENT),
            MeasureDef::Radius(k) | MeasureDef::Circumference(k) | MeasureDef::CircleArea(k) => {
                c.expect(k, CIRCLE)
            }
            MeasureDef::Slope(l) => c.expect(l, LINE_LIKE),
            MeasureDef::DistancePointPath { point, path } => {
                c.expect(point, POINT)?;
                c.expect(path, LINE_LIKE)
            }
            MeasureDef::DistanceLines(g, h) => {
                c.expect(g, LINE_LIKE)?;
                c.expect(h, LINE_LIKE)
            }
            MeasureDef::Area3(a, b, d) | MeasureDef::AffineRatio(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            MeasureDef::Angle(a, b, d) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)
            }
            MeasureDef::Area4(a, b, d, e) | MeasureDef::CrossRatio(a, b, d, e) => {
                c.expect(a, POINT)?;
                c.expect(b, POINT)?;
                c.expect(d, POINT)?;
                c.expect(e, POINT)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        let number = |v: f64| Some(Value::Number(v));
        match *self {
            MeasureDef::Length(s) => match c.value(s) {
                Value::Segment { x1, y1, x2, y2 } => number(segment_length(x1, y1, x2, y2)),
                _ => None,
            },
            MeasureDef::Radius(k) => number(circle_radius(c, k)?),
            MeasureDef::Circumference(k) => {
                let r = circle_radius(c, k)?;
                // A circle of radius 0 is a single point: no circumference.
                if r == 0.0 {
                    None
                } else {
                    number(2.0 * r * PI)
                }
            }
            MeasureDef::CircleArea(k) => {
                let r = circle_radius(c, k)?;
                number(r * r * PI)
            }
            MeasureDef::Area3(a, b, d) => {
                let pts = [c.point(a)?, c.point(b)?, c.point(d)?];
                number(polygon_area(&pts).abs())
            }
            MeasureDef::Area4(a, b, d, e) => {
                let pts = [c.point(a)?, c.point(b)?, c.point(d)?, c.point(e)?];
                number(polygon_area(&pts).abs())
            }
            MeasureDef::Slope(l) => {
                let (x, y, _) = raw_line(c, l)?;
                number(slope(x, y)?)
            }
            MeasureDef::DistancePointPath { point, path } => {
                let p = c.point(point)?;
                match c.value(path) {
                    Value::Line { a, b, c: z } => number((a * p.0 + b * p.1 + z).abs()),
                    Value::Segment { x1, y1, x2, y2 } => {
                        number(point_segment_distance(p, x1, y1, x2, y2))
                    }
                    _ => None,
                }
            }
            MeasureDef::DistanceLines(g, h) => {
                number(line_line_distance(raw_line(c, g)?, raw_line(c, h)?))
            }
            MeasureDef::AffineRatio(a, b, d) => {
                let (pa, pb, pd) = (c.point(a)?, c.point(b)?, c.point(d)?);
                if !collinear(pa, pb, pd) {
                    return None;
                }
                if same_point(pb, pd) {
                    return number(1.0);
                }
                number(affine_ratio(pa, pb, pd))
            }
            MeasureDef::CrossRatio(a, b, d, e) => {
                let (pa, pb, pd, pe) = (c.point(a)?, c.point(b)?, c.point(d)?, c.point(e)?);
                if same_point(pa, pe)
                    || same_point(pb, pd)
                    || !collinear(pb, pd, pe)
                    || !collinear(pa, pd, pe)
                {
                    return None;
                }
                number(affine_ratio(pb, pd, pe) / affine_ratio(pa, pd, pe))
            }
            MeasureDef::Angle(a, b, d) => {
                let (pa, pb, pd) = (c.point(a)?, c.point(b)?, c.point(d)?);
                let (vx, vy) = (pa.0 - pb.0, pa.1 - pb.1);
                let (wx, wy) = (pd.0 - pb.0, pd.1 - pb.1);
                if (is_zero(vx) && is_zero(vy)) || (is_zero(wx) && is_zero(wy)) {
                    return None;
                }
                let det = vx * wy - vy * wx;
                let prod = vx * wx + vy * wy;
                number(angle_value(det.atan2(prod)))
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        let n = args.len();
        if func == "Distance" {
            return parse_distance(c, args);
        }
        let known = matches!(
            func,
            "Length"
                | "Radius"
                | "Circumference"
                | "Perimeter"
                | "Area"
                | "Slope"
                | "AffineRatio"
                | "CrossRatio"
                | "Angle"
        );
        if !known {
            return None;
        }
        let wanted = match func {
            "Area" => matches!(n, 1 | 3 | 4),
            "AffineRatio" | "Angle" => n == 3,
            "CrossRatio" => n == 4,
            _ => n == 1,
        };
        if !wanted {
            return Some(Err(format!("`{func}` does not take {n} argument(s)")));
        }
        let mut ids = [Id(0); 4];
        for (slot, arg) in ids.iter_mut().zip(args) {
            match object(c, arg) {
                Ok(id) => *slot = id,
                Err(e) => return Some(Err(e)),
            }
        }
        let [a, b, d, e] = ids;
        Some(Ok(match (func, n) {
            ("Length", _) => MeasureDef::Length(a),
            ("Radius", _) => MeasureDef::Radius(a),
            ("Circumference" | "Perimeter", _) => MeasureDef::Circumference(a),
            ("Area", 1) => MeasureDef::CircleArea(a),
            ("Area", 3) => MeasureDef::Area3(a, b, d),
            ("Area", _) => MeasureDef::Area4(a, b, d, e),
            ("Slope", _) => MeasureDef::Slope(a),
            ("AffineRatio", _) => MeasureDef::AffineRatio(a, b, d),
            ("CrossRatio", _) => MeasureDef::CrossRatio(a, b, d, e),
            _ => MeasureDef::Angle(a, b, d),
        }))
    }
}

/// `Distance(point, line or segment)`, either order, and `Distance(line, line)`.
/// Every other shape (two points, a circle, an unknown object) is left to the
/// built-in command, which also reports the errors.
fn parse_distance(c: &Construction, args: &[&str]) -> Option<Result<MeasureDef, String>> {
    if args.len() != 2 {
        return None;
    }
    let (first, second) = (c.id(args[0])?, c.id(args[1])?);
    let line_like = |id: Id| matches!(c.kind(id), Kind::Line | Kind::Segment);
    let def = match (c.kind(first), c.kind(second)) {
        (Kind::Point, _) if line_like(second) => MeasureDef::DistancePointPath {
            point: first,
            path: second,
        },
        (_, Kind::Point) if line_like(first) => MeasureDef::DistancePointPath {
            point: second,
            path: first,
        },
        _ if line_like(first) && line_like(second) => MeasureDef::DistanceLines(first, second),
        _ => return None,
    };
    Some(Ok(def))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Def;
    use crate::script::{exec, load};

    fn num(c: &Construction, label: &str) -> Option<f64> {
        match c.value(c.id(label).unwrap()) {
            Value::Number(v) => Some(v),
            Value::Undefined => None,
            other => panic!("{label} is {other:?}"),
        }
    }

    fn close(a: Option<f64>, b: f64) {
        let a = a.expect("defined");
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }

    const BASE: &str = "A = (0, 0)\nB = (4, 0)\nP = (1, 3)\nC = (0, 3)\n\
                        s = Segment(A, B)\nl = Line(A, B)\nk = Circle(A, B)\n";

    #[test]
    fn every_syntax_parses() {
        let mut src = String::from(BASE);
        for line in [
            "d1 = Length(s)",
            "d2 = Radius(k)",
            "d3 = Circumference(k)",
            "d4 = Perimeter(k)",
            "d5 = Area(k)",
            "d6 = Area(A, B, P)",
            "d7 = Area(A, B, P, C)",
            "d8 = Slope(l)",
            "d9 = Slope(s)",
            "e1 = Distance(P, l)",
            "e2 = Distance(P, s)",
            "e3 = Distance(l, P)",
            "e4 = Distance(l, s)",
            "e5 = AffineRatio(A, B, A)",
            "e6 = CrossRatio(A, B, A, B)",
            "e7 = Angle(B, A, C)",
        ] {
            src.push_str(line);
            src.push('\n');
        }
        let c = load(&src).unwrap();
        assert!(matches!(c.def(c.id("d1").unwrap()), Def::Measure(_)));
        close(num(&c, "d1"), 4.0);
        close(num(&c, "d2"), 4.0);
        close(num(&c, "d3"), 8.0 * PI);
        close(num(&c, "d4"), 8.0 * PI);
        close(num(&c, "d5"), 16.0 * PI);
        close(num(&c, "d6"), 6.0);
        close(num(&c, "d7"), 7.5);
        close(num(&c, "d8"), 0.0);
        close(num(&c, "d9"), 0.0);
        close(num(&c, "e1"), 3.0);
        close(num(&c, "e2"), 3.0);
        close(num(&c, "e3"), 3.0);
        close(num(&c, "e4"), 0.0);
        close(num(&c, "e7"), PI / 2.0);
    }

    #[test]
    fn distance_of_two_points_is_left_to_the_built_in_command() {
        let c = load("A = (0, 0)\nB = (3, 4)\nd = Distance(A, B)\n").unwrap();
        assert!(matches!(c.def(c.id("d").unwrap()), Def::Distance(..)));
        close(num(&c, "d"), 5.0);
    }

    #[test]
    fn wrong_argument_counts_and_kinds_are_errors() {
        let mut c = load(BASE).unwrap();
        for (line, text) in [
            ("d = Length(s, s)", "`Length` does not take 2 argument(s)"),
            ("d = Radius(k, k)", "`Radius` does not take 2 argument(s)"),
            ("d = Area(A, B)", "`Area` does not take 2 argument(s)"),
            (
                "d = Area(A, B, P, C, A)",
                "`Area` does not take 5 argument(s)",
            ),
            ("d = Slope(l, l)", "`Slope` does not take 2 argument(s)"),
            ("d = AffineRatio(A, B)", "`AffineRatio` does not take 2"),
            ("d = CrossRatio(A, B, P)", "`CrossRatio` does not take 3"),
            ("d = Angle(A, B)", "`Angle` does not take 2 argument(s)"),
            ("d = Distance(A, B, P)", "`Distance` does not take 3"),
            ("d = Length(l)", "`l` is a line, expected a segment"),
            ("d = Length(A)", "`A` is a point, expected a segment"),
            ("d = Radius(s)", "`s` is a segment, expected a circle"),
            ("d = Area(A)", "`A` is a point, expected a circle"),
            (
                "d = Slope(k)",
                "`k` is a circle, expected a line or segment",
            ),
            ("d = Area(A, B, k)", "`k` is a circle, expected a point"),
            ("d = Angle(A, l, B)", "`l` is a line, expected a point"),
            ("d = Distance(P, k)", "`k` is a circle"),
            ("d = Length(Z)", "unknown object `Z`"),
        ] {
            let err = exec(&mut c, line).unwrap_err();
            assert!(err.contains(text), "{line}: {err}");
        }
    }

    #[test]
    fn degenerate_inputs() {
        let mut c = load(
            "A = (0, 0)\nB = (4, 0)\nC = (0, 3)\nr = 2\n\
             s = Segment(A, B)\nl = Line(A, B)\nk = Circle(A, r)\n\
             sl = Slope(l)\nss = Slope(s)\nci = Circumference(k)\nar = Area(k)\n\
             tri = Area(A, B, C)\nang = Angle(B, A, C)\nlen = Length(s)\n",
        )
        .unwrap();
        let a = c.id("A").unwrap();
        let b = c.id("B").unwrap();
        let r = c.id("r").unwrap();

        // Vertical line and segment have no slope.
        c.move_point(b, 0.0, 5.0).unwrap();
        assert_eq!(num(&c, "sl"), None);
        assert_eq!(num(&c, "ss"), None);
        close(num(&c, "len"), 5.0);

        // Coincident end points: zero length, no line, no angle at the vertex.
        c.move_point(b, 0.0, 0.0).unwrap();
        assert_eq!(num(&c, "sl"), None);
        assert_eq!(num(&c, "ss"), None);
        close(num(&c, "len"), 0.0);
        assert_eq!(num(&c, "ang"), None);
        close(num(&c, "tri"), 0.0);
        c.move_point(b, 4.0, 0.0).unwrap();
        c.move_point(a, 4.0, 0.0).unwrap();
        assert_eq!(num(&c, "ang"), None);
        c.move_point(a, 0.0, 0.0).unwrap();

        // A circle of radius 0 is a point: no circumference, area 0.
        c.set_number(r, 0.0).unwrap();
        assert_eq!(num(&c, "ci"), None);
        close(num(&c, "ar"), 0.0);
        c.set_number(r, 1.0).unwrap();
        close(num(&c, "ci"), 2.0 * PI);
    }

    #[test]
    fn a_radius_below_the_tolerance_is_a_single_point() {
        let c = load(
            "A = (0, 0)\nr = 0.000000005\nk = Circle(A, r)\n\
             d = Radius(k)\np = Circumference(k)\na = Area(k)\n",
        )
        .unwrap();
        close(num(&c, "d"), 0.0);
        assert_eq!(num(&c, "p"), None);
        close(num(&c, "a"), 0.0);
    }

    #[test]
    fn angle_is_counter_clockwise_in_zero_to_two_pi() {
        let mut c =
            load("A = (1, 0)\nB = (0, 0)\nC = (0, 1)\nw = Angle(A, B, C)\nu = Angle(C, B, A)\n")
                .unwrap();
        close(num(&c, "w"), PI / 2.0);
        close(num(&c, "u"), 1.5 * PI);
        let ptc = c.id("C").unwrap();
        c.move_point(ptc, 1.0, 0.0).unwrap();
        close(num(&c, "w"), 0.0);
        c.move_point(ptc, -1.0, 0.0).unwrap();
        close(num(&c, "w"), PI);
        c.move_point(ptc, 1.0, -1.0).unwrap();
        close(num(&c, "w"), 1.75 * PI);
    }

    #[test]
    fn ratios_need_collinear_points() {
        let mut c = load(
            "A = (0, 0)\nB = (2, 0)\nC = (6, 0)\nD = (3, 0)\n\
             r = AffineRatio(A, B, C)\nq = CrossRatio(A, B, C, D)\n",
        )
        .unwrap();
        close(num(&c, "r"), 3.0);
        let (ptc, ptd) = (c.id("C").unwrap(), c.id("D").unwrap());
        // (B, C, D) / (A, C, D) = ((3-2)/(6-2)) / ((3-0)/(6-0)).
        close(num(&c, "q"), 0.25 / 0.5);
        c.move_point(ptc, 6.0, 1.0).unwrap();
        assert_eq!(num(&c, "r"), None);
        assert_eq!(num(&c, "q"), None);
        c.move_point(ptc, 2.0, 0.0).unwrap();
        close(num(&c, "r"), 1.0);
        assert_eq!(num(&c, "q"), None);
        c.move_point(ptc, 6.0, 0.0).unwrap();
        c.move_point(ptd, 0.0, 0.0).unwrap();
        assert_eq!(num(&c, "q"), None);
    }

    #[test]
    fn zero_tests_use_the_raw_scale_of_the_lines() {
        // sin(angle) is 1e-10: parallel for unit normals, but the raw cross
        // product is about 1e-2, so GeoGebra sees intersecting lines.
        let c = load(
            "A = (0, 0)\nB = (10000, 0)\nC = (0, 1)\nD = (10000, 1.000001)\n\
             l = Line(A, B)\nm = Line(C, D)\nd = Distance(l, m)\n\
             F = (0.001, 1000000)\nv = Line(A, F)\nt = Slope(v)\n",
        )
        .unwrap();
        close(num(&c, "d"), 0.0);
        close(num(&c, "t"), 1e9);
    }
}
