// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdLineBisector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdAngularBisector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdTangent.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/advanced/CmdPolar.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLineBisector.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoLineBisectorSegment.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAngularBisectorPoints.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoAngularBisectorLines.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPointND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLineND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoTangentLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/TangentAlgo.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoPolarLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersectLineConic.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/algos/AlgoIntersect.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoConicND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/kernelND/GeoQuadricND.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoVec3D.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoLine.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/geos/GeoPoint.java
//   source/shared/common/src/main/java/org/geogebra/common/util/DoubleUtil.java
//   source/shared/common/src/main/java/org/geogebra/common/util/MyMath.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Lines built from other objects: perpendicular and angle bisectors,
//! tangents to circles and polar lines.
//!
//! GeoGebra computes these lines with raw (not normalised) coefficients and
//! decides "undefined" on those raw values; the rules below build the same
//! raw triple and keep it (see [`Value::Line`]).

use super::{
    check_index, line_value, normalized_coefficients, Group, Parents, CIRCLE, LINE_LIKE, POINT,
};
use crate::kernel::{Construction, Error, Id, Kind, Value};
use crate::script::object;
use crate::tol::{MAX_PRECISION, MIN_PRECISION, STANDARD_PRECISION};

/// Lines built from other objects: bisectors, tangents and polars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineDef {
    /// Perpendicular bisector of two points (`LineBisector(A, B)`).
    BisectorPoints(Id, Id),
    /// Perpendicular bisector of a segment (`LineBisector(s)`).
    BisectorSegment(Id),
    /// Bisector of the angle at `vertex` (`AngularBisector(A, B, C)`).
    AngleBisectorPoints { leg1: Id, vertex: Id, leg2: Id },
    /// One of the two angle bisectors of two lines or segments
    /// (`AngularBisector(g, h)`); `which` is 0 or 1 in GeoGebra's order.
    AngleBisectorLines { g: Id, h: Id, which: u8 },
    /// One of the two tangents from a point to a circle
    /// (`Tangent(P, c)`); `which` is 0 or 1 in GeoGebra's order.
    TangentPoint { point: Id, circle: Id, which: u8 },
    /// One of the two tangents to a circle parallel to a line or segment
    /// (`Tangent(g, c)`); `which` is 0 or 1 in GeoGebra's order.
    TangentLine { line: Id, circle: Id, which: u8 },
    /// Polar line of a point with respect to a circle (`Polar(P, c)`).
    Polar { point: Id, circle: Id },
}

impl Group for LineDef {
    fn kind(&self) -> Kind {
        Kind::Line
    }

    fn parents(&self) -> Parents {
        match *self {
            LineDef::BisectorPoints(p, q) => Parents::of(&[p, q]),
            LineDef::BisectorSegment(s) => Parents::of(&[s]),
            LineDef::AngleBisectorPoints { leg1, vertex, leg2 } => {
                Parents::of(&[leg1, vertex, leg2])
            }
            LineDef::AngleBisectorLines { g, h, .. } => Parents::of(&[g, h]),
            LineDef::TangentPoint { point, circle, .. } | LineDef::Polar { point, circle } => {
                Parents::of(&[point, circle])
            }
            LineDef::TangentLine { line, circle, .. } => Parents::of(&[line, circle]),
        }
    }

    fn check(&self, c: &Construction) -> Result<(), Error> {
        match *self {
            LineDef::BisectorPoints(p, q) => {
                c.expect(p, POINT)?;
                c.expect(q, POINT)
            }
            LineDef::BisectorSegment(s) => c.expect(s, SEGMENT),
            LineDef::AngleBisectorPoints { leg1, vertex, leg2 } => {
                c.expect(leg1, POINT)?;
                c.expect(vertex, POINT)?;
                c.expect(leg2, POINT)
            }
            LineDef::AngleBisectorLines { g, h, which } => {
                c.expect(g, LINE_LIKE)?;
                c.expect(h, LINE_LIKE)?;
                check_index(which)
            }
            LineDef::TangentPoint {
                point,
                circle,
                which,
            } => {
                c.expect(point, POINT)?;
                c.expect(circle, CIRCLE)?;
                check_index(which)
            }
            LineDef::TangentLine {
                line,
                circle,
                which,
            } => {
                c.expect(line, LINE_LIKE)?;
                c.expect(circle, CIRCLE)?;
                check_index(which)
            }
            LineDef::Polar { point, circle } => {
                c.expect(point, POINT)?;
                c.expect(circle, CIRCLE)
            }
        }
    }

    fn compute(&self, c: &Construction) -> Option<Value> {
        match *self {
            LineDef::BisectorPoints(p, q) => {
                let (ax, ay) = c.point(p)?;
                let (bx, by) = c.point(q)?;
                line_bisector(ax, ay, bx, by)
            }
            LineDef::BisectorSegment(s) => match c.value(s) {
                Value::Segment { x1, y1, x2, y2 } => line_bisector(x1, y1, x2, y2),
                _ => None,
            },
            LineDef::AngleBisectorPoints { leg1, vertex, leg2 } => {
                angle_bisector_points(c.point(leg1)?, c.point(vertex)?, c.point(leg2)?)
            }
            LineDef::AngleBisectorLines { g, h, which } => {
                angle_bisector_lines(c.raw_carrier(g)?, c.raw_carrier(h)?, which)
            }
            LineDef::TangentPoint {
                point,
                circle,
                which,
            } => tangent_point(c.point(point)?, c.circle(circle)?, which),
            LineDef::TangentLine {
                line,
                circle,
                which,
            } => tangent_line(c.raw_carrier(line)?, c.circle(circle)?, which),
            LineDef::Polar { point, circle } => {
                let (px, py) = c.point(point)?;
                let (cx, cy, r) = c.circle(circle)?;
                let polar = CircleMatrix::new(cx, cy, r).polar(px, py);
                line_value(polar.0, polar.1, polar.2)
            }
        }
    }

    fn parse(c: &Construction, func: &str, args: &[&str]) -> Option<Result<Self, String>> {
        Some(match func {
            "LineBisector" | "PerpendicularBisector" => parse_line_bisector(c, func, args),
            "AngularBisector" | "AngleBisector" => match args.len() {
                3 => {
                    let ids = match objects(c, args) {
                        Ok(ids) => ids,
                        Err(e) => return Some(Err(e)),
                    };
                    expect_kinds(c, func, &ids, &[Kind::Point, Kind::Point, Kind::Point]).map(
                        |()| LineDef::AngleBisectorPoints {
                            leg1: ids[0],
                            vertex: ids[1],
                            leg2: ids[2],
                        },
                    )
                }
                // two lines: `parse_multi`
                2 => return None,
                n => Err(arg_count(func, n)),
            },
            // two arguments: `parse_multi`
            "Tangent" if args.len() == 2 => return None,
            "Tangent" => Err(arg_count(func, args.len())),
            "Polar" => parse_polar(c, func, args),
            _ => return None,
        })
    }

    fn parse_multi(
        c: &Construction,
        func: &str,
        args: &[&str],
    ) -> Option<Result<Vec<Self>, String>> {
        match (func, args.len()) {
            ("AngularBisector" | "AngleBisector", 2) => Some(objects(c, args).and_then(|ids| {
                let (g, h) = (ids[0], ids[1]);
                if is_line_like(c.kind(g)) && is_line_like(c.kind(h)) {
                    Ok((0..2)
                        .map(|which| LineDef::AngleBisectorLines { g, h, which })
                        .collect())
                } else {
                    Err(bad_kinds(c, func, &ids))
                }
            })),
            ("Tangent", 2) => Some(objects(c, args).and_then(|ids| {
                let (first, second) = (ids[0], ids[1]);
                match (c.kind(first), c.kind(second)) {
                    (Kind::Point, Kind::Circle) => Ok(tangent_points(c, first, second)),
                    (Kind::Circle, Kind::Point) => Ok(tangent_points(c, second, first)),
                    (Kind::Line | Kind::Segment, Kind::Circle) => Ok((0..2)
                        .map(|which| LineDef::TangentLine {
                            line: first,
                            circle: second,
                            which,
                        })
                        .collect()),
                    (Kind::Circle, Kind::Circle) => {
                        Err("common tangents of two circles are not supported yet".into())
                    }
                    _ => Err(bad_kinds(c, func, &ids)),
                }
            })),
            _ => None,
        }
    }
}

const SEGMENT: &[Kind] = &[Kind::Segment];

/// The tangents from `point` to `circle`. As in the constructor of
/// `AlgoTangentPointND`, when both tangents are the same line at creation
/// (the point is on the circle) the second one is made undefined. The
/// kernel then creates only the first tangent, and does not add the second
/// one later when the point leaves the circle.
fn tangent_points(c: &Construction, point: Id, circle: Id) -> Vec<LineDef> {
    let defs: Vec<LineDef> = (0..2)
        .map(|which| LineDef::TangentPoint {
            point,
            circle,
            which,
        })
        .collect();
    match (defs[0].compute(c), defs[1].compute(c)) {
        (Some(t0), Some(t1)) if lin_dep(t0, t1) => defs[..1].to_vec(),
        _ => defs,
    }
}

/// `GeoVec3D.linDep` for two lines: the same line within the standard
/// precision.
pub(crate) fn lin_dep(l1: Value, l2: Value) -> bool {
    let (Value::Line { a: x, b: y, c: z }, Value::Line { a: u, b: v, c: w }) = (l1, l2) else {
        return false;
    };
    let n1 = x.abs().max(y.abs().max(z.abs()));
    let n2 = u.abs().max(v.abs().max(w.abs()));
    let (x1, y1, z1) = (x / n1, y / n1, z / n1);
    let (x2, y2, z2) = (u / n2, v / n2, w / n2);
    let eq = |p: f64, q: f64| is_equal(p, q, STANDARD_PRECISION);
    eq(x1 * y2, x2 * y1) && eq(z1 * y2, z2 * y1) && eq(x1 * z2, x2 * z1)
}

fn parse_line_bisector(c: &Construction, func: &str, args: &[&str]) -> Result<LineDef, String> {
    match args.len() {
        1 => {
            let s = object(c, args[0])?;
            expect_kinds(c, func, &[s], &[Kind::Segment]).map(|()| LineDef::BisectorSegment(s))
        }
        2 => {
            let ids = objects(c, args)?;
            expect_kinds(c, func, &ids, &[Kind::Point, Kind::Point])
                .map(|()| LineDef::BisectorPoints(ids[0], ids[1]))
        }
        n => Err(arg_count(func, n)),
    }
}

fn parse_polar(c: &Construction, func: &str, args: &[&str]) -> Result<LineDef, String> {
    if args.len() != 2 {
        return Err(arg_count(func, args.len()));
    }
    let ids = objects(c, args)?;
    match (c.kind(ids[0]), c.kind(ids[1])) {
        (Kind::Point, Kind::Circle) => Ok(LineDef::Polar {
            point: ids[0],
            circle: ids[1],
        }),
        (Kind::Line | Kind::Segment, Kind::Circle) => {
            Err("the pole of a line (`Polar` of a line and a circle) is not supported yet".into())
        }
        _ => Err(bad_kinds(c, func, &ids)),
    }
}

fn objects(c: &Construction, args: &[&str]) -> Result<Vec<Id>, String> {
    args.iter().map(|a| object(c, a)).collect()
}

fn is_line_like(k: Kind) -> bool {
    matches!(k, Kind::Line | Kind::Segment)
}

fn expect_kinds(c: &Construction, func: &str, ids: &[Id], kinds: &[Kind]) -> Result<(), String> {
    if ids.iter().zip(kinds).all(|(&id, &k)| c.kind(id) == k) {
        Ok(())
    } else {
        Err(bad_kinds(c, func, ids))
    }
}

fn bad_kinds(c: &Construction, func: &str, ids: &[Id]) -> String {
    let kinds: Vec<String> = ids.iter().map(|&id| format!("a {}", c.kind(id))).collect();
    format!("`{func}` does not take {}", kinds.join(", "))
}

fn arg_count(func: &str, n: usize) -> String {
    format!("`{func}` does not take {n} argument(s)")
}

// ------------------------------------------------------------------ numerics

/// `DoubleUtil.isZero`: strictly inside the standard precision.
fn is_zero(v: f64) -> bool {
    -STANDARD_PRECISION < v && v < STANDARD_PRECISION
}

/// `DoubleUtil.isEqual(x, y, eps)`.
fn is_equal(x: f64, y: f64, eps: f64) -> bool {
    x == y || ((x - eps) < y && y < (x + eps))
}

/// `MyMath.length`, kept bit-exact because the angle bisectors compare
/// signs of values derived from it.
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

/// `GeoVec3D.cross(u, v, w)`, including its rescaling by `w.z`.
fn cross(u: (f64, f64, f64), v: (f64, f64, f64)) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = (
        u.1 * v.2 - u.2 * v.1,
        u.2 * v.0 - u.0 * v.2,
        u.0 * v.1 - u.1 * v.0,
    );
    // 1e15 is GeoGebra's `Kernel.INV_MAX_DOUBLE_PRECISION`.
    if (z.is_finite() && z.abs() > 1e15) || (z != 0.0 && z.abs() < MAX_PRECISION) {
        x /= z;
        y /= z;
        z = 1.0;
    }
    (x, y, z)
}

/// `AlgoLineBisector.compute`.
fn line_bisector(ax: f64, ay: f64, bx: f64, by: f64) -> Option<Value> {
    let x = ax - bx;
    let y = ay - by;
    // The midpoint is (ax + bx, ay + by, 2) in homogeneous coordinates.
    let z = -((ax + bx) * x + (ay + by) * y) / 2.0;
    line_value(x, y, z)
}

/// Unit direction `(gx, gy)` of a raw line, as both angle bisectors use it.
fn direction(g: (f64, f64, f64)) -> (f64, f64, f64) {
    let (gx, gy) = (g.1, -g.0);
    let len = my_length(gx, gy);
    (gx / len, gy / len, len)
}

/// Direction of the inner bisector of two unit directions, as in both
/// `AlgoAngularBisector*` classes when the vertex is finite.
fn bisector_direction(gx: f64, gy: f64, hx: f64, hy: f64) -> (f64, f64) {
    let ip = gx * hx + gy * hy;
    let (mut wx, mut wy);
    if ip >= 0.0 {
        // angle < 90 degrees
        wx = gx + hx;
        wy = gy + hy;
    } else {
        // angle > 90 degrees: BC - BA is a normal vector of the bisector
        wx = hy - gy;
        wy = gx - hx;
        // angle > 180 degrees: change the orientation
        if gx * hy < gy * hx {
            wx = -wx;
            wy = -wy;
        }
    }
    let len = my_length(wx, wy);
    (wx / len, wy / len)
}

/// `AlgoAngularBisectorPoints.compute` for a finite vertex `b`
/// (continuity off).
fn angle_bisector_points(a: (f64, f64), b: (f64, f64), cc: (f64, f64)) -> Option<Value> {
    let bh = (b.0, b.1, 1.0);
    let g = cross(bh, (a.0, a.1, 1.0));
    let h = cross(bh, (cc.0, cc.1, 1.0));
    let (gx, gy, _) = direction(g);
    let (hx, hy, _) = direction(h);
    let (wx, wy) = bisector_direction(gx, gy, hx, hy);
    let (x, y) = (-wy, wx);
    line_value(x, y, -(b.0 * x + b.1 * y))
}

/// `AlgoAngularBisectorLines.compute` (continuity off). `which` selects
/// GeoGebra's `bisector[which]`.
fn angle_bisector_lines(g: (f64, f64, f64), h: (f64, f64, f64), which: u8) -> Option<Value> {
    // The intersection B of g and h, as `GeoVec3D.cross` and
    // `GeoPoint.updateCoords` see it: infinite when z is negligible.
    let (bx, by, bz) = (
        g.1 * h.2 - g.2 * h.1,
        g.2 * h.0 - g.0 * h.2,
        g.0 * h.1 - g.1 * h.0,
    );
    let infinite = {
        let e = bz.abs();
        !(e > STANDARD_PRECISION
            || e > bx.abs() * STANDARD_PRECISION
            || e > by.abs() * STANDARD_PRECISION)
    };
    let (gx, gy, len_g) = direction(g);
    let (hx, hy, len_h) = direction(h);
    if infinite {
        // Parallel lines: one bisector in the middle, the other undefined.
        let (index, z) = if gx * hx + gy * hy > 0.0 {
            (0, (g.2 / len_g + h.2 / len_h) / 2.0)
        } else {
            (1, (g.2 / len_g - h.2 / len_h) / 2.0)
        };
        if which != index {
            return None;
        }
        return line_value(-gy, gx, z);
    }
    // inhomogeneous coordinates of B
    let (px, py) = if bz == 1.0 {
        (bx, by)
    } else {
        (bx / bz, by / bz)
    };
    let (wx, wy) = bisector_direction(gx, gy, hx, hy);
    // bisector[1] is perpendicular to bisector[0]: direction (-wy, wx)
    let (x, y) = if which == 0 { (-wy, wx) } else { (-wx, -wy) };
    line_value(x, y, -(px * x + py * y))
}

/// A circle as GeoGebra stores it: centre, radius and the flat matrix
/// `[1, 1, cx² + cy² - r², 0, -cx, -cy]` of `GeoQuadricND.setSphereNDMatrix`.
struct CircleMatrix {
    cx: f64,
    cy: f64,
    r: f64,
    m: [f64; 6],
}

impl CircleMatrix {
    fn new(cx: f64, cy: f64, r: f64) -> Self {
        let mut m2 = -r * r;
        m2 += cx * cx;
        m2 += cy * cy;
        CircleMatrix {
            cx,
            cy,
            r,
            m: [1.0, 1.0, m2, 0.0, -cx, -cy],
        }
    }

    /// A radius below the standard precision makes GeoGebra's circle a
    /// single point, a degenerate conic without tangents. A circle built by
    /// `Circle(...)` takes its type in `GeoQuadricND.setSphereNDMatrix`
    /// from this test on the radius, not from the conic classification.
    fn is_degenerate(&self) -> bool {
        self.r < STANDARD_PRECISION
    }

    /// `GeoConicND.polarLine` for the point `(px, py, 1)`.
    fn polar(&self, px: f64, py: f64) -> (f64, f64, f64) {
        let m = &self.m;
        (
            m[0] * px + m[3] * py + m[4],
            m[3] * px + m[1] * py + m[5],
            m[4] * px + m[5] * py + m[2],
        )
    }

    /// `GeoConicND.diameterLine` for the direction `(vx, vy)`.
    fn diameter(&self, vx: f64, vy: f64) -> (f64, f64, f64) {
        let m = &self.m;
        (
            m[0] * vx + m[3] * vy,
            m[3] * vx + m[1] * vy,
            m[4] * vx + m[5] * vy,
        )
    }

    /// `GeoConicND.evaluate(matrix, x, y)`.
    fn evaluate(&self, x: f64, y: f64) -> f64 {
        let m = &self.m;
        m[2] + m[4] * x
            + m[5] * y
            + y * (m[5] + m[3] * x + m[1] * y)
            + x * (m[4] + m[0] * x + m[3] * y)
    }

    /// `GeoConicND.isOnFullConic` for a circle.
    fn is_on(&self, px: f64, py: f64, eps: f64) -> bool {
        let (x, y) = (px - self.cx, py - self.cy);
        let r2 = self.r * self.r;
        is_equal(x * x / r2 + y * y / r2, 1.0, eps)
    }
}

/// `GeoLine.isOnFullLine` for a finite point.
fn is_on_line(g: (f64, f64, f64), px: f64, py: f64, eps: f64) -> bool {
    (g.0 * px + g.1 * py + g.2).abs() < eps * (g.0.abs() + g.1.abs())
}

type Pt = Option<(f64, f64)>;

/// `AlgoIntersectLineConic.intersectLineConic` for a circle: the points in
/// the order of increasing parameter along the direction `(y, -x)`.
fn intersect_line_circle(g: (f64, f64, f64), a: &CircleMatrix, eps: f64) -> [Pt; 2] {
    let (x, y, z) = normalized_coefficients(g, 2.0, 0.5);
    let m = &a.m;
    // an arbitrary point of the line
    let (px, py) = if x.abs() > y.abs() {
        (-z / x, 0.0)
    } else {
        (0.0, -z / y)
    };
    let sv_x = m[0] * y - m[3] * x;
    let sv_y = m[3] * y - m[1] * x;
    let u = y * sv_x - x * sv_y;
    let d = px * sv_x + py * sv_y + m[4] * y - m[5] * x;
    let w = a.evaluate(px, py);
    let delta = MIN_PRECISION.min(1f64.max((2.0 * d).abs() + u.abs() + w.abs()) * eps);
    let at = |t: f64| Some((px + t * y, py - t * x));

    if -eps < u && u < eps {
        if -eps < d && d < eps {
            // degenerate or asymptote
            return [None, None];
        }
        // the line meets the conic once
        let t0 = -w / (2.0 * d);
        return if d < 0.0 {
            [at(t0), None]
        } else {
            [None, at(t0)]
        };
    }
    let mut dis = d * d - u * w;
    if is_equal(dis, 0.0, delta) {
        // tangent line
        let t1 = -d / u;
        return [at(t1), at(t1)];
    }
    if dis > 0.0 {
        dis = dis.sqrt();
        // compute one root with (-d +/- dis) / u and the other with
        // w / (-d +/- dis), choosing the sign that makes d + dis larger
        let swap = d < 0.0;
        if swap {
            dis = -dis;
        }
        let q = -(d + dis);
        let t1 = if swap { w / q } else { q / u };
        let t2 = if swap { q / u } else { w / q };
        return [at(t1), at(t2)];
    }
    [None, None]
}

/// `AlgoIntersectLineConic.intersect` followed by `avoidDoubleTangentPoint`,
/// with the identity permutation that a tangent algorithm keeps when its
/// two tangent points are distinct at creation.
fn tangent_points_of(g: (f64, f64, f64), a: &CircleMatrix) -> [Pt; 2] {
    if g.0.is_nan() || g.1.is_nan() || g.2.is_nan() || (is_zero(g.0) && is_zero(g.1)) {
        return [None, None];
    }
    let mut sol = [None, None];
    let mut ok = false;
    let mut eps = 1e-15;
    while eps <= MIN_PRECISION {
        sol = intersect_line_circle(g, a, eps);
        // `testPoints`: keep the points that lie on the line and the circle
        for p in sol.iter_mut() {
            if let Some((x, y)) = *p {
                if x.is_nan() || y.is_nan() {
                    *p = None;
                } else if a.is_on(x, y, MIN_PRECISION) && is_on_line(g, x, y, MIN_PRECISION) {
                    ok = true;
                } else {
                    *p = None;
                }
            }
        }
        if ok {
            break;
        }
        eps *= 10.0;
    }
    if !ok {
        return [None, None];
    }
    if let (Some(p), Some(q)) = (sol[0], sol[1]) {
        if is_equal(p.0, q.0, STANDARD_PRECISION) && is_equal(p.1, q.1, STANDARD_PRECISION) {
            sol[1] = None;
        }
    }
    sol
}

/// The point where the tangent `d` touches `circle`, as
/// `TangentAlgo.getTangentPoint` gives it to `AlgoIntersectLineConic`:
/// `None` when `d` is not a tangent of `circle`, `Some(None)` when the
/// point is undefined.
pub(crate) fn tangent_point_on(c: &Construction, d: LineDef, circle: Id) -> Option<Pt> {
    match d {
        LineDef::TangentPoint {
            point,
            circle: k,
            which,
        } if k == circle => Some((|| {
            let (px, py) = c.point(point)?;
            let (cx, cy, r) = c.circle(k)?;
            let a = CircleMatrix::new(cx, cy, r);
            if a.is_degenerate() {
                return None;
            }
            if a.is_on(px, py, STANDARD_PRECISION) {
                // the tangent points of a point on the circle are the point
                return Some((px, py));
            }
            tangent_points_of(a.polar(px, py), &a)[which as usize]
        })()),
        LineDef::TangentLine {
            line,
            circle: k,
            which,
        } if k == circle => Some((|| {
            let g = c.raw_carrier(line)?;
            let (cx, cy, r) = c.circle(k)?;
            let a = CircleMatrix::new(cx, cy, r);
            if a.is_degenerate() {
                return None;
            }
            tangent_points_of(a.diameter(g.1, -g.0), &a)[which as usize]
        })()),
        _ => None,
    }
}

/// `AlgoTangentPointND.compute` with `AlgoTangentPoint` for a circle.
fn tangent_point(p: (f64, f64), circle: (f64, f64, f64), which: u8) -> Option<Value> {
    let a = CircleMatrix::new(circle.0, circle.1, circle.2);
    if a.is_degenerate() {
        return None;
    }
    let polar = a.polar(p.0, p.1);
    if a.is_on(p.0, p.1, STANDARD_PRECISION) {
        // the polar of a point on the circle is its tangent
        return line_value(polar.0, polar.1, polar.2);
    }
    let (tx, ty) = tangent_points_of(polar, &a)[which as usize]?;
    // `GeoVec3D.lineThroughPoints(P, T)`
    line_value(p.1 - ty, tx - p.0, p.0 * ty - p.1 * tx)
}

/// `AlgoTangentLineND.compute` with `AlgoTangentLine` for a circle.
fn tangent_line(g: (f64, f64, f64), circle: (f64, f64, f64), which: u8) -> Option<Value> {
    let a = CircleMatrix::new(circle.0, circle.1, circle.2);
    if a.is_degenerate() {
        return None;
    }
    // the diameter conjugate to the direction (g.y, -g.x)
    let diameter = a.diameter(g.1, -g.0);
    let (tx, ty) = tangent_points_of(diameter, &a)[which as usize]?;
    line_value(g.0, g.1, -(tx * g.0 + ty * g.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Def;
    use crate::script::load;

    const BASE: &str = "A = (0, 0)\nB = (4, 0)\nC = (1, 3)\nP = (5, 1)\nr = 2\n\
                        k = Circle(A, r)\nl = Line(A, B)\nm = Line(A, C)\ns = Segment(A, C)\n";

    fn build(extra: &str) -> Construction {
        load(&format!("{BASE}{extra}")).unwrap()
    }

    fn err(extra: &str) -> String {
        load(&format!("{BASE}{extra}")).err().unwrap().message
    }

    fn def(c: &Construction, label: &str) -> LineDef {
        match c.def(c.id(label).unwrap()) {
            Def::Lines(d) => d,
            other => panic!("{label} is {other:?}"),
        }
    }

    fn line(c: &Construction, label: &str) -> (f64, f64, f64) {
        match c.value(c.id(label).unwrap()) {
            Value::Line { a, b, c } => (a, b, c),
            other => panic!("{label} is {other:?}"),
        }
    }

    /// Compares with GeoGebra's raw coefficients, scale and sign included.
    fn assert_line(c: &Construction, label: &str, raw: (f64, f64, f64)) {
        let (a, b, cc) = line(c, label);
        for (got, want) in [(a, raw.0), (b, raw.1), (cc, raw.2)] {
            assert!((got - want).abs() < 1e-12, "{label}: {got} != {want}");
        }
    }

    #[test]
    fn parses_every_syntax() {
        let c = build(
            "f = LineBisector(A, B)\ng = PerpendicularBisector(s)\n\
             h = AngularBisector(A, B, C)\nn = AngleBisector(C, B, A)\n\
             b = AngularBisector(l, m)\nw = AngleBisector(s, l)\n\
             t = Tangent(P, k)\nu = Tangent(k, P)\nv = Tangent(l, k)\nq = Tangent(s, k)\n\
             p = Polar(P, k)\n",
        );
        let id = |s: &str| c.id(s).unwrap();
        assert_eq!(def(&c, "f"), LineDef::BisectorPoints(id("A"), id("B")));
        assert_eq!(def(&c, "g"), LineDef::BisectorSegment(id("s")));
        assert!(matches!(def(&c, "h"), LineDef::AngleBisectorPoints { .. }));
        assert!(matches!(def(&c, "n"), LineDef::AngleBisectorPoints { .. }));
        for (label, which) in [("b_{1}", 0), ("b_{2}", 1)] {
            assert_eq!(
                def(&c, label),
                LineDef::AngleBisectorLines {
                    g: id("l"),
                    h: id("m"),
                    which
                }
            );
        }
        assert!(matches!(
            def(&c, "w_{2}"),
            LineDef::AngleBisectorLines { .. }
        ));
        for t in ["t", "u"] {
            for (k, which) in [(1, 0), (2, 1)] {
                assert_eq!(
                    def(&c, &format!("{t}_{{{k}}}")),
                    LineDef::TangentPoint {
                        point: id("P"),
                        circle: id("k"),
                        which
                    }
                );
            }
        }
        assert!(matches!(
            def(&c, "v_{1}"),
            LineDef::TangentLine { which: 0, .. }
        ));
        assert!(matches!(
            def(&c, "q_{2}"),
            LineDef::TangentLine { which: 1, .. }
        ));
        assert_eq!(
            def(&c, "p"),
            LineDef::Polar {
                point: id("P"),
                circle: id("k")
            }
        );
        for label in ["b", "t", "u", "v", "w", "q"] {
            assert!(c.id(label).is_none(), "{label} should not exist");
        }
    }

    #[test]
    fn rejects_wrong_argument_counts() {
        for (line, n) in [
            ("f = LineBisector(A, B, C)", 3),
            ("f = PerpendicularBisector(A, B, C, P)", 4),
            ("f = AngularBisector(A)", 1),
            ("f = AngleBisector(A, B, C, P)", 4),
            ("f = Tangent(P)", 1),
            ("f = Tangent(P, k, A)", 3),
            ("f = Polar(P)", 1),
        ] {
            let e = err(line);
            assert!(
                e.contains(&format!("does not take {n} argument(s)")),
                "{line}: {e}"
            );
        }
    }

    #[test]
    fn rejects_wrong_kinds() {
        for (line, part) in [
            ("f = LineBisector(A)", "does not take a point"),
            ("f = LineBisector(A, l)", "does not take a point, a line"),
            ("f = AngularBisector(A, l)", "does not take a point, a line"),
            (
                "f = AngularBisector(A, B, l)",
                "does not take a point, a point, a line",
            ),
            ("f = Tangent(A, B)", "does not take a point, a point"),
            ("f = Tangent(k, l)", "does not take a circle, a line"),
            ("f = Tangent(k, k)", "not supported"),
            ("f = Polar(l, k)", "not supported"),
            ("f = Polar(A, B)", "does not take a point, a point"),
            ("f = Tangent(P, Z)", "unknown object `Z`"),
        ] {
            let e = err(line);
            assert!(e.contains(part), "{line}: {e}");
        }
    }

    #[test]
    fn values_match_geogebra() {
        // Raw coefficients printed by GeoGebra for the same construction.
        let c = build(
            "f = LineBisector(A, B)\nh = AngularBisector(B, A, C)\nt = Tangent(P, k)\n\
             v = Tangent(l, k)\np = Polar(C, k)\n",
        );
        assert_line(&c, "f", (-4.0, 0.0, 8.0));
        assert_line(
            &c,
            "t_{1}",
            (-0.957852215316704, -4.591570443063341, 9.380831519646861),
        );
        assert_line(
            &c,
            "t_{2}",
            (2.6501599076243956, -3.869968018475121, -9.38083151964686),
        );
        assert_line(&c, "v_{1}", (0.0, 4.0, -8.0));
        assert_line(&c, "v_{2}", (0.0, 4.0, 8.0));
        assert_line(&c, "p", (1.0, 3.0, -4.0));
        let (a, b, cc) = line(&c, "h");
        assert!(a < 0.0 && b > 0.0 && cc.abs() < 1e-15, "{a} {b} {cc}");
    }

    #[test]
    fn degenerate_inputs_are_undefined() {
        let mut c = build("t = Tangent(P, k)\nf = LineBisector(A, P)\np = Polar(P, k)\n");
        let undefined = |c: &Construction, s: &str| !c.value(c.id(s).unwrap()).is_defined();
        let p = c.id("P").unwrap();
        // inside the circle: no tangents
        c.move_point(p, 0.5, 0.5).unwrap();
        assert!(undefined(&c, "t_{1}") && undefined(&c, "t_{2}"));
        // on the circle: both tangents are the tangent at P
        c.move_point(p, 0.0, 2.0).unwrap();
        assert_eq!(line(&c, "t_{1}"), line(&c, "t_{2}"));
        // the polar of P: GeoGebra's raw coefficients (0, 2, -4)
        assert_eq!(line(&c, "t_{1}"), (0.0, 2.0, -4.0));
        // at the centre: no polar, no perpendicular bisector
        c.move_point(p, 0.0, 0.0).unwrap();
        assert!(undefined(&c, "p") && undefined(&c, "f"));
        // radius zero: no tangents
        c.move_point(p, 5.0, 1.0).unwrap();
        c.set_number(c.id("r").unwrap(), 0.0).unwrap();
        assert!(undefined(&c, "t_{1}") && undefined(&c, "t_{2}"));
    }
}
